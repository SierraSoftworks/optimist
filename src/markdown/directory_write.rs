use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::{DirectoryError, RenderedSnapshot, directory_error};

/// Publishes a complete rendered snapshot through a sibling staging directory.
///
/// Files are written before publication, so partial rendering or writes never
/// alter the destination. Replacing an existing directory uses a backup rename
/// and attempts rollback if staging publication fails. A successful call removes
/// stale generated files because only snapshot members enter the new directory.
///
/// ```no_run
/// use optimist::markdown::{RenderedSnapshot, read_directory, write_directory};
///
/// let imported = read_directory("model")?;
/// let snapshot = RenderedSnapshot::from_import(&imported)?;
/// write_directory("model-copy", &snapshot)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write_directory(
    path: impl AsRef<Path>,
    snapshot: &RenderedSnapshot,
) -> Result<(), DirectoryError> {
    let destination = path.as_ref();
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| directory_error::io("create parent for", destination, error))?;
    let token = Uuid::new_v4();
    let staging = sibling(destination, &format!("staging-{token}"));
    let backup = sibling(destination, &format!("backup-{token}"));
    if let Err(error) = create_staging(&staging, snapshot) {
        remove_staging(&staging);
        return Err(error);
    }

    if destination.exists() {
        if let Err(error) = std::fs::rename(destination, &backup) {
            remove_staging(&staging);
            return Err(directory_error::io(
                "move existing export from",
                destination,
                error,
            ));
        }
        if let Err(error) = std::fs::rename(&staging, destination) {
            let publish_error = directory_error::io("publish", destination, error);
            if let Err(rollback) = std::fs::rename(&backup, destination) {
                return Err(DirectoryError::Io {
                    operation: "publish and restore",
                    path: destination.to_owned(),
                    message: format!("{publish_error}; rollback failed: {rollback}"),
                });
            }
            return Err(publish_error);
        }
        std::fs::remove_dir_all(&backup)
            .map_err(|error| directory_error::io("remove backup", &backup, error))?;
    } else {
        std::fs::rename(&staging, destination)
            .map_err(|error| directory_error::io("publish", destination, error))?;
    }
    Ok(())
}

fn create_staging(path: &Path, snapshot: &RenderedSnapshot) -> Result<(), DirectoryError> {
    std::fs::create_dir(path)
        .map_err(|error| directory_error::io("create staging", path, error))?;
    for (relative, content) in snapshot.files() {
        let target = path.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| directory_error::io("create", parent, error))?;
        }
        std::fs::write(&target, content)
            .map_err(|error| directory_error::io("write", target, error))?;
    }
    Ok(())
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("optimist-export");
    path.with_file_name(format!(".{name}.{suffix}"))
}

fn remove_staging(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}
