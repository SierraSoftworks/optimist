use std::path::{Path, PathBuf};

use thiserror::Error;
use uuid::Uuid;

use super::{
    ImportError, RenderedSnapshot, SourceDocument, ValidatedImport, YamlError, parse_entity,
    parse_project, parse_scenario,
};

const MAX_COLLECTION_FILES: usize = 10_000;

/// Failures while reading or publishing a YAML project directory.
#[derive(Debug, Error)]
pub enum DirectoryError {
    /// A filesystem operation failed at a source-aware path.
    #[error("could not {operation} {path}: {message}")]
    Io {
        /// Operation attempted by the directory transport.
        operation: &'static str,
        /// Filesystem path at which the operation failed.
        path: PathBuf,
        /// Operating-system diagnostic.
        message: String,
    },
    /// A bounded YAML file failed local parsing or rendering.
    #[error(transparent)]
    Yaml(#[from] YamlError),
    /// The complete parsed collection failed project-level validation.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// A document directory exceeds the collection-level file bound.
    #[error("{path}: YAML project exceeds the {maximum} file limit")]
    TooManyFiles {
        /// Directory whose entries exceeded the bound.
        path: PathBuf,
        /// Maximum accepted entity and scenario document count.
        maximum: usize,
    },
    /// Legacy Markdown project files are not accepted by the YAML importer.
    #[error("{0}: Markdown project files are unsupported; export the project as YAML")]
    UnsupportedMarkdown(PathBuf),
}

/// Reads, bounds, parses, and cross-validates one YAML project directory.
pub fn read_directory(path: impl AsRef<Path>) -> Result<ValidatedImport, DirectoryError> {
    let root = path.as_ref();
    reject_markdown(root)?;
    let project_path = root.join("_project.yaml");
    let project = SourceDocument::new(
        "_project.yaml",
        parse_project("_project.yaml", &read_text(&project_path)?)?,
    );
    let entity_paths = yaml_files(&root.join("entities"), MAX_COLLECTION_FILES)?;
    let scenario_paths = yaml_files(
        &root.join("scenarios"),
        MAX_COLLECTION_FILES - entity_paths.len(),
    )?;
    let entities = entity_paths
        .into_iter()
        .map(|file| {
            let relative = relative_path(root, &file);
            let document = parse_entity(relative.clone(), &read_text(&file)?)?;
            if document.canonical_path() != relative {
                return Err(io_message(
                    "validate canonical path for",
                    file,
                    format!("expected {}", document.canonical_path()),
                ));
            }
            Ok(SourceDocument::new(relative, document))
        })
        .collect::<Result<_, DirectoryError>>()?;
    let scenarios = scenario_paths
        .into_iter()
        .map(|file| {
            let relative = relative_path(root, &file);
            let document = parse_scenario(relative.clone(), &read_text(&file)?)?;
            if document.canonical_path() != relative {
                return Err(io_message(
                    "validate canonical path for",
                    file,
                    format!("expected {}", document.canonical_path()),
                ));
            }
            Ok(SourceDocument::new(relative, document))
        })
        .collect::<Result<_, DirectoryError>>()?;
    Ok(ValidatedImport::new(project, entities, scenarios)?)
}

/// Atomically publishes a complete rendered YAML snapshot.
pub fn write_directory(
    path: impl AsRef<Path>,
    snapshot: &RenderedSnapshot,
) -> Result<(), DirectoryError> {
    let destination = path.as_ref();
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| io("create parent for", destination, error))?;
    let token = Uuid::new_v4();
    let staging = sibling(destination, &format!("staging-{token}"));
    let backup = sibling(destination, &format!("backup-{token}"));
    if let Err(error) = create_staging(&staging, snapshot) {
        remove_staging(&staging);
        return Err(error);
    }
    if destination.exists() {
        std::fs::rename(destination, &backup)
            .map_err(|error| io("move existing export from", destination, error))?;
        if let Err(error) = std::fs::rename(&staging, destination) {
            let publish_error = io("publish", destination, error);
            if let Err(rollback) = std::fs::rename(&backup, destination) {
                return Err(io_message(
                    "publish and restore",
                    destination,
                    format!("{publish_error}; rollback failed: {rollback}"),
                ));
            }
            return Err(publish_error);
        }
        std::fs::remove_dir_all(&backup).map_err(|error| io("remove backup", &backup, error))?;
    } else {
        std::fs::rename(&staging, destination)
            .map_err(|error| io("publish", destination, error))?;
    }
    Ok(())
}

fn yaml_files(path: &Path, maximum: usize) -> Result<Vec<PathBuf>, DirectoryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(path).map_err(|error| io("list", path, error))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io("read entry in", path, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| io("inspect", entry.path(), error))?;
        if file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|value| value == "yaml")
        {
            files.push(entry.path());
            if files.len() > maximum {
                return Err(DirectoryError::TooManyFiles {
                    path: path.to_owned(),
                    maximum,
                });
            }
        }
    }
    files.sort();
    Ok(files)
}

fn reject_markdown(root: &Path) -> Result<(), DirectoryError> {
    for directory in [
        root.to_owned(),
        root.join("entities"),
        root.join("scenarios"),
    ] {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(io("list", directory, error)),
        };
        for entry in entries {
            let path = entry
                .map_err(|error| io("read entry in", &directory, error))?
                .path();
            if path.extension().is_some_and(|extension| extension == "md") {
                return Err(DirectoryError::UnsupportedMarkdown(path));
            }
        }
    }
    Ok(())
}

fn create_staging(path: &Path, snapshot: &RenderedSnapshot) -> Result<(), DirectoryError> {
    std::fs::create_dir(path).map_err(|error| io("create staging", path, error))?;
    for (relative, content) in snapshot.files() {
        let target = path.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io("create", parent, error))?;
        }
        std::fs::write(&target, content).map_err(|error| io("write", target, error))?;
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, DirectoryError> {
    std::fs::read_to_string(path).map_err(|error| io("read", path, error))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("discovered below the supplied root")
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
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

fn io(operation: &'static str, path: impl Into<PathBuf>, error: std::io::Error) -> DirectoryError {
    io_message(operation, path, error.to_string())
}

fn io_message(
    operation: &'static str,
    path: impl Into<PathBuf>,
    message: String,
) -> DirectoryError {
    DirectoryError::Io {
        operation,
        path: path.into(),
        message,
    }
}
