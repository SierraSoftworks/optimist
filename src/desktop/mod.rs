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
//! [`feed`] carries the change feed a socket would have carried, [`archive`]
//! swaps a browser's download and upload for the platform's own file dialogs,
//! and [`workspace`] lets somebody say where their designs live.

mod archive;
mod bridge;
mod failure;
mod feed;
mod frontend;
mod settings;
mod workspace;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard,
        atomic::{AtomicU32, Ordering},
    },
};

use tauri::{Manager, RunEvent, async_runtime::JoinHandle};
use tokio::{
    runtime::{Handle, Runtime},
    task::JoinHandle as Sweeping,
};

use crate::{
    api::{self, Service},
    session::Workspace,
};

use failure::Failure;

/// Everything the commands share.
///
/// The folder can be changed while the window is open, because somebody who
/// keeps their designs elsewhere should not have to restart to say so.
/// Everything derived from it — the caches, the solve board, the feeds, the
/// sweep that writes edits out — belongs to the folder that produced it and is
/// replaced along with it.
pub(crate) struct Desktop {
    open: RwLock<Open>,
    /// What each subscription is running, so that it can be ended by number.
    feeds: Mutex<HashMap<u32, JoinHandle<()>>>,
    subscriptions: AtomicU32,
    /// Where the sweep for a newly opened folder is started.
    runtime: Handle,
}

/// One folder, and everything the application has built over it.
struct Open {
    folder: PathBuf,
    service: Arc<Service>,
    sweeping: Sweeping<()>,
}

impl Desktop {
    /// Opens a folder. Needs a runtime available for the sweep to run on.
    fn new(folder: PathBuf) -> Self {
        Self {
            open: RwLock::new(Open::over(folder)),
            feeds: Mutex::default(),
            subscriptions: AtomicU32::new(0),
            runtime: Handle::current(),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, Open> {
        self.open.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// The API over the folder currently open.
    fn service(&self) -> Arc<Service> {
        Arc::clone(&self.read().service)
    }

    /// Where designs are being read from and written to.
    fn folder(&self) -> PathBuf {
        self.read().folder.clone()
    }

    /// Opens another folder in place of the one in use.
    ///
    /// What the old folder still held is written out first, and everything
    /// watching it is stopped, because a feed over a design that is no longer on
    /// screen would go on reporting changes nobody asked about.
    fn open(&self, folder: PathBuf) -> std::io::Result<()> {
        prepare(&folder)?;
        let mut open = self.open.write().unwrap_or_else(PoisonError::into_inner);
        for (_, watching) in self.feeds().drain() {
            watching.abort();
        }
        open.close();
        let _running = self.runtime.enter();
        *open = Open::over(folder);
        Ok(())
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

impl Open {
    fn over(folder: PathBuf) -> Self {
        let workspace = Arc::new(Workspace::new(&folder));
        let sweeping = api::sweep(Arc::clone(&workspace));
        Self {
            folder,
            service: Arc::new(Service::new(workspace)),
            sweeping,
        }
    }

    /// Writes out everything still unsaved and stops writing anything more.
    fn close(&self) {
        self.sweeping.abort();
        if let Err(error) = self.service.workspace().persist_all() {
            eprintln!("unsaved designs could not be written: {error}");
        }
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
    prepare(&folder).map_err(|error| {
        human_errors::user(
            format!("{} could not be created: {error}", folder.display()),
            &[
                "Choose a folder you can write to with --designs.",
                "Check that the path is not a file, and that the disk is not full.",
            ],
        )
    })?;

    tauri::async_runtime::set(runtime.handle().clone());
    let _running = runtime.enter();

    let mut context = tauri::generate_context!();
    context.set_assets(Box::new(frontend::Workbench::new(None)));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Desktop::new(folder.clone()))
        .invoke_handler(tauri::generate_handler![
            bridge::api_call,
            feed::feed_subscribe,
            feed::feed_unsubscribe,
            archive::export_design,
            archive::choose_archive,
            archive::import_design,
            workspace::workspace_folder,
            workspace::choose_workspace,
        ])
        .setup(move |app| {
            if unasked {
                settings::offer(app.handle().clone(), folder.clone());
            }
            Ok(())
        })
        .build(context)
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
            app.state::<Desktop>().read().close();
        }
    });
    Ok(())
}

/// Makes sure a folder exists before anything is asked of it.
fn prepare(folder: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A scratch directory that takes itself away again.
    pub(crate) struct Scratch(PathBuf);

    impl Scratch {
        /// Somewhere to put files a test needs, beside the workspace itself.
        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }

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
        let root = scratch();
        let designs = root.path().join("designs");
        std::fs::create_dir_all(&designs).expect("a scratch workspace");
        (Desktop::new(designs), root)
    }

    /// A directory of its own, for a test that needs somewhere to put files.
    pub(crate) fn scratch() -> Scratch {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!(
            "optimist-desktop-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("a scratch directory");
        Scratch(root)
    }
}
