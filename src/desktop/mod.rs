//! The workbench in a window.
//!
//! # Why there is no server
//!
//! The application could have started the same HTTP server `optimist serve`
//! does and pointed a webview at it, which would have been less code. It would
//! also have opened a port on the machine that every other process on it could
//! reach, holding an editable copy of somebody's designs behind no
//! authentication at all, for the sake of a loopback round trip per keystroke.
//!
//! Requests therefore arrive over Tauri's IPC, which nothing outside this
//! process can speak, and are put through the same router the server exposes.
//! Nothing about a design is answered twice: [`bridge`] routes requests,
//! [`feed`] carries the change feed a socket would have carried, and [`archive`]
//! swaps a browser's download and upload for the platform's own file dialogs.

mod archive;
mod bridge;
mod failure;
mod feed;
mod settings;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU32, Ordering},
    },
};

use tauri::{AppHandle, Manager, RunEvent, async_runtime::JoinHandle};
use tokio::runtime::Runtime;

use crate::{
    api::{self, Service},
    session::Workspace,
};

use failure::Failure;

/// Everything the commands share.
pub(crate) struct Desktop {
    service: Service,
    /// What each subscription is running, so that it can be ended by number.
    feeds: Mutex<HashMap<u32, JoinHandle<()>>>,
    subscriptions: AtomicU32,
}

impl Desktop {
    fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            service: Service::new(workspace),
            feeds: Mutex::default(),
            subscriptions: AtomicU32::new(0),
        }
    }

    /// A number no live subscription is using.
    fn subscription(&self) -> u32 {
        self.subscriptions.fetch_add(1, Ordering::Relaxed)
    }

    /// A poisoned registry is still a correct list of what is running.
    fn feeds(&self) -> MutexGuard<'_, HashMap<u32, JoinHandle<()>>> {
        self.feeds.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Opens the workbench over a folder of designs.
///
/// The runtime is passed in rather than built here because Tauri owns the main
/// thread once it starts, and everything the API does — the solves, the timers,
/// the sweep that writes designs out — has to already have somewhere to run.
pub(crate) fn run(designs: Option<PathBuf>, runtime: Runtime) -> Result<(), human_errors::Error> {
    let remembered = settings::Settings::load();
    let unasked = designs.is_none() && remembered.designs.is_none();
    let folder = designs
        .or(remembered.designs)
        .unwrap_or_else(settings::default_designs);
    settings::prepare(&folder)?;

    tauri::async_runtime::set(runtime.handle().clone());
    let _running = runtime.enter();

    let workspace = Arc::new(Workspace::new(&folder));
    let sweeping = Arc::clone(&workspace);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Desktop::new(workspace))
        .invoke_handler(tauri::generate_handler![
            bridge::api_call,
            feed::feed_subscribe,
            feed::feed_unsubscribe,
            archive::save_archive,
            archive::choose_archive,
            archive::import_archive,
        ])
        .setup(move |app| {
            api::sweep(sweeping);
            if unasked {
                settings::offer(app.handle().clone(), folder.clone());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .map_err(|error| {
            human_errors::system(
                format!("The application could not start: {error}"),
                &[
                    "Check that this machine has a graphical session to open a window in.",
                    "Run `optimist serve` and use the workbench in a browser instead.",
                ],
            )
        })?;

    app.run(|app, event| {
        // An edit is held briefly before it is written, and closing the window
        // is exactly the moment somebody stops waiting for that.
        if matches!(event, RunEvent::Exit) {
            persist(app);
        }
    });
    Ok(())
}

/// Writes out everything still unsaved.
fn persist(app: &AppHandle) {
    if let Err(error) = app.state::<Desktop>().service.workspace().persist_all() {
        eprintln!("unsaved designs could not be written: {error}");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use super::*;

    /// A workspace directory that takes itself away again.
    pub(crate) struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// An empty workspace, and the directory holding it.
    ///
    /// The directory is returned rather than kept, because it lives exactly as
    /// long as the test binding it does.
    pub(crate) fn workspace() -> (Desktop, Scratch) {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "optimist-desktop-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("a scratch workspace");
        (Desktop::new(Arc::new(Workspace::new(&root))), Scratch(root))
    }
}
