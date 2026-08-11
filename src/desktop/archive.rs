//! Designs leaving and arriving as files.
//!
//! A browser downloads an archive and uploads one through a form; a window asks
//! where to put the file and reads the one it was handed. Only those two ends
//! differ, so both still go through the API, and a design imported here is
//! stored exactly as one imported over HTTP would be.
//!
//! Nothing but a path crosses the IPC boundary. The file is read and written
//! where it lives, rather than being copied through JSON on its way to a process
//! that could have opened it directly.

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::{session::DesignId, system::MAX_ARCHIVE_BYTES};

use super::{Desktop, Failure, bridge};

/// What a filter in a file dialog is called.
const KIND: &str = "Design archive";

/// An archive somebody chose, named by where it is rather than by its bytes.
#[derive(Debug, Serialize)]
pub(super) struct Chosen {
    name: String,
    path: PathBuf,
}

#[tauri::command]
pub(super) async fn export_design(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    design: String,
) -> Result<(), Failure> {
    let (id, archive) = packed(desktop.inner(), design).await?;

    // A command runs off the main thread, which is where a blocking dialog has
    // to be asked from.
    let Some(chosen) = app
        .dialog()
        .file()
        .set_file_name(format!("{id}.zip"))
        .add_filter(KIND, &["zip"])
        .blocking_save_file()
    else {
        return Ok(());
    };

    let path = located(chosen)?;
    std::fs::write(&path, archive).map_err(|error| {
        Failure::fault(
            format!("{} could not be written: {error}", path.display()),
            "Choose a folder you can write to, then export the design again.",
        )
    })
}

#[tauri::command]
pub(super) async fn choose_archive(app: AppHandle) -> Result<Option<Chosen>, Failure> {
    let Some(chosen) = app
        .dialog()
        .file()
        .add_filter(KIND, &["zip"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };

    let path = located(chosen)?;
    Ok(Some(Chosen {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path,
    }))
}

#[tauri::command]
pub(super) async fn import_design(
    desktop: State<'_, Desktop>,
    path: PathBuf,
    design: String,
    replace: bool,
) -> Result<Value, Failure> {
    store(desktop.inner(), &path, design, replace).await
}

async fn store(
    desktop: &Desktop,
    path: &Path,
    design: String,
    replace: bool,
) -> Result<Value, Failure> {
    let id = identifier(design)?;
    let contents = read(path)?;
    let query = if replace { "?replace=true" } else { "" };
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/v1/designs/{id}/archive{query}"))
        .header(header::CONTENT_TYPE, "application/zip")
        .body(Body::from(contents))
        .expect("an identifier the workspace accepted is a path");

    let stored = bridge::read(bridge::route(desktop, request).await?).await?;
    serde_json::from_slice(&stored).map_err(|error| {
        Failure::fault(
            format!("The stored design could not be read back: {error}"),
            "This is a defect in the application rather than in the archive.",
        )
    })
}

/// Reads a design out as the file it would have been downloaded as.
async fn packed(desktop: &Desktop, design: String) -> Result<(DesignId, Vec<u8>), Failure> {
    let id = identifier(design)?;
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/designs/{id}/archive"))
        .body(Body::empty())
        .expect("an identifier the workspace accepted is a path");
    let archive = bridge::read(bridge::route(desktop, request).await?).await?;
    Ok((id, archive))
}

/// Reads an archive, refusing one too large to be a design before reading it.
fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    let size = std::fs::metadata(path).map(|file| file.len()).unwrap_or(0);
    if size > MAX_ARCHIVE_BYTES {
        return Err(Failure::refused(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("{} is larger than a design archive may be.", path.display()),
            "Check that this is a design archive rather than some other zip.",
        ));
    }
    std::fs::read(path).map_err(|error| {
        Failure::fault(
            format!("{} could not be read: {error}", path.display()),
            "Check that the file is still there and can be read, then choose it again.",
        )
    })
}

/// A dialog may name something the system only describes, such as a cloud file.
fn located(chosen: FilePath) -> Result<PathBuf, Failure> {
    chosen.into_path().map_err(|error| {
        Failure::fault(
            format!("The chosen file could not be reached: {error}"),
            "Choose a file on this machine rather than one only the system can name.",
        )
    })
}

/// Checks the name before it reaches a path.
///
/// Everything above builds a URI by writing the identifier into it, and only an
/// identifier the workspace has already accepted is safe to write there.
fn identifier(design: String) -> Result<DesignId, Failure> {
    DesignId::new(design).map_err(|error| {
        Failure::refused(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "Use the directory name of a design, in lower case.",
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::desktop::{bridge, tests::workspace};

    use super::*;

    /// Writes an archive out the way the save dialog would have.
    async fn exported(desktop: &Desktop, design: &str, into: &Path) -> PathBuf {
        let (id, archive) = packed(desktop, design.to_owned())
            .await
            .expect("packs the design");
        let path = into.join(format!("{id}.zip"));
        std::fs::write(&path, archive).expect("writes the archive");
        path
    }

    /// A design exported from the window is one the window can take back.
    #[tokio::test]
    async fn packs_a_design_it_can_store_again() {
        let (desktop, root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;

        let path = exported(&desktop, "checkout", root.path()).await;
        let stored = store(&desktop, &path, "billing".to_owned(), false)
            .await
            .expect("stores it");

        assert_eq!(stored["name"], "checkout");
    }

    /// Replacing a design loses what it held, so it is never done by accident.
    #[tokio::test]
    async fn refuses_to_replace_a_design_unless_it_is_told_to() {
        let (desktop, root) = workspace();
        bridge::tests::create(&desktop, "checkout").await;
        let path = exported(&desktop, "checkout", root.path()).await;

        let failure = store(&desktop, &path, "checkout".to_owned(), false)
            .await
            .expect_err("is refused");
        assert_eq!(
            serde_json::to_value(&failure).expect("serialises")["status"],
            409
        );

        store(&desktop, &path, "checkout".to_owned(), true)
            .await
            .expect("replaces it when asked to");
    }

    #[tokio::test]
    async fn refuses_a_name_that_could_not_be_a_directory() {
        let (desktop, root) = workspace();
        let path = root.path().join("anything.zip");

        let failure = store(&desktop, &path, "../elsewhere".to_owned(), false)
            .await
            .expect_err("is refused");
        assert_eq!(
            serde_json::to_value(&failure).expect("serialises")["status"],
            400
        );
    }

    #[tokio::test]
    async fn refuses_something_that_is_not_an_archive() {
        let (desktop, root) = workspace();
        let path = root.path().join("broken.zip");
        std::fs::write(&path, b"not a zip").expect("writes the file");

        let failure = store(&desktop, &path, "checkout".to_owned(), false)
            .await
            .expect_err("is refused");

        let reported = serde_json::to_value(&failure).expect("serialises");
        assert_eq!(reported["status"], 422);
        assert!(!reported["advice"].as_array().expect("advice").is_empty());
    }

    #[tokio::test]
    async fn reports_a_file_it_cannot_read() {
        let (desktop, root) = workspace();
        let path = root.path().join("missing.zip");

        assert!(
            store(&desktop, &path, "checkout".to_owned(), false)
                .await
                .is_err()
        );
    }
}
