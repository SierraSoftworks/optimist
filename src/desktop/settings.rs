//! Where designs are kept, and how somebody changes their mind about it.
//!
//! A window launched from a desktop has no working directory worth reading, so
//! the default is a folder under Documents where a person can find, back up and
//! share what they have written. It is offered rather than imposed: the first
//! launch says where designs are going and gives them somewhere else to say.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

/// What the folder is called under Documents, and the application under config.
const NAME: &str = "optimist";

/// What the application remembers between launches.
#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct Settings {
    /// The folder designs are read from and written to, once somebody has said.
    #[serde(default)]
    pub(super) designs: Option<PathBuf>,
}

impl Settings {
    /// Reads what was remembered, treating anything unreadable as nothing.
    ///
    /// A settings file that cannot be parsed is a file this application wrote
    /// and can write again, and refusing to start over it would strand somebody
    /// with no way back in.
    pub(super) fn load() -> Self {
        file()
            .and_then(|path| fs::read(path).ok())
            .and_then(|stored| serde_json::from_slice(&stored).ok())
            .unwrap_or_default()
    }

    fn store(&self) {
        let Some(path) = file() else { return };
        let Some(parent) = path.parent() else { return };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        if let Ok(rendered) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(path, rendered);
        }
    }
}

fn file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join(NAME).join("settings.json"))
}

/// Where designs go when nobody has said otherwise.
pub(super) fn default_designs() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(NAME)
}

/// Says where designs are going, and offers somewhere else to put them.
///
/// Asked once, on the launch that had nothing remembered. Accepting the folder
/// remembers it so the question is not asked again; changing it opens the other
/// one there and then. Somebody who opens the picker and changes their mind is
/// asked again next time, having answered nothing.
pub(super) fn offer(app: AppHandle, current: PathBuf) {
    // Off the main thread, which is where the event loop the dialog needs runs.
    std::thread::spawn(move || {
        let keep = app
            .dialog()
            .message(format!(
                "Your designs will be kept in {}.",
                current.display()
            ))
            .title("Where your designs live")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Use this folder".to_owned(),
                "Choose another…".to_owned(),
            ))
            .blocking_show();

        if keep {
            remember(current);
            return;
        }

        let chosen = app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|folder| folder.into_path().ok());
        if let Some(chosen) = chosen {
            super::workspace::adopted(&app, chosen);
        }
    });
}

/// Records the folder to open next time.
pub(super) fn remember(designs: PathBuf) {
    Settings {
        designs: Some(designs),
    }
    .store();
}
