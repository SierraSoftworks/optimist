//! Which folder of designs the window is looking at.
//!
//! Changing it swaps the workspace underneath rather than restarting, so the
//! answer to "where do you keep your designs" is a question a person answers
//! once and can revisit without losing the window they are working in.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use super::{Desktop, Failure, settings};

#[tauri::command]
pub(super) fn workspace_folder(desktop: State<'_, Desktop>) -> String {
    desktop.folder().display().to_string()
}

#[tauri::command]
pub(super) async fn choose_workspace(
    app: AppHandle,
    desktop: State<'_, Desktop>,
) -> Result<Option<String>, Failure> {
    // A command runs off the main thread, which is where a blocking dialog has
    // to be asked from.
    let Some(chosen) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let folder = chosen.into_path().map_err(|error| {
        Failure::fault(
            format!("The chosen folder could not be reached: {error}"),
            "Choose a folder on this machine rather than one only the system can name.",
        )
    })?;

    adopt(desktop.inner(), folder.clone())?;
    Ok(Some(folder.display().to_string()))
}

/// Opens a folder and remembers it as the one to open next time.
pub(super) fn adopt(desktop: &Desktop, folder: PathBuf) -> Result<(), Failure> {
    desktop.open(folder.clone()).map_err(|error| {
        Failure::fault(
            format!("{} could not be opened: {error}", folder.display()),
            "Choose a folder you can write to.",
        )
    })?;
    settings::remember(folder);
    Ok(())
}

/// Opens a folder chosen from somewhere that only holds the application.
pub(super) fn adopted(app: &AppHandle, folder: PathBuf) {
    if let Err(error) = adopt(&app.state::<Desktop>(), folder) {
        eprintln!("the chosen folder could not be opened: {error:?}");
    }
}

#[cfg(test)]
mod tests {
    use crate::desktop::{bridge, tests::workspace};

    /// The window is looking at whatever it was last told to look at.
    #[tokio::test]
    async fn opens_another_folder_without_restarting() {
        let (desktop, root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;

        let elsewhere = root.path().join("elsewhere");
        desktop.open(elsewhere.clone()).expect("opens it");

        assert_eq!(desktop.folder(), elsewhere);
        let designs = bridge::call(&desktop, "GET", "/api/v1/designs", None)
            .await
            .expect("lists designs");
        assert!(
            designs.as_array().expect("a list").is_empty(),
            "a design in the folder that was closed is not in the one that was opened"
        );
    }

    /// An edit still being held is written out before the folder is let go of.
    #[tokio::test]
    async fn writes_out_what_the_old_folder_still_held() {
        let (desktop, root) = workspace();
        let designs = desktop.folder();
        bridge::tests::create(&desktop, "checkout").await;

        desktop
            .open(root.path().join("elsewhere"))
            .expect("opens it");

        assert!(
            designs.join("checkout").join("_system.yaml").is_file(),
            "the design was written before the folder was closed"
        );
    }

    /// A feed over the old folder would report changes nobody asked about.
    #[tokio::test]
    async fn stops_watching_the_folder_it_leaves() {
        let (desktop, root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;
        bridge::tests::watch(&desktop, "checkout").await;
        assert_eq!(desktop.feeds().len(), 1);

        desktop
            .open(root.path().join("elsewhere"))
            .expect("opens it");

        assert!(desktop.feeds().is_empty());
    }
}
