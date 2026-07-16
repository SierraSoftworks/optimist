use std::path::{Path, PathBuf};

use super::{
    DirectoryError, SourceDocument, ValidatedImport, directory_error, parse_entity, parse_project,
    parse_scenario,
};

const MAX_COLLECTION_FILES: usize = 10_000;

/// Reads, bounds, parses, and cross-validates one Markdown project directory.
///
/// Only `_project.md`, `entities/*.md`, and `scenarios/*.md` participate. Each
/// file remains subject to the parser's independent document/frontmatter limits.
pub fn read_directory(path: impl AsRef<Path>) -> Result<ValidatedImport, DirectoryError> {
    let root = path.as_ref();
    let project_path = root.join("_project.md");
    let project = SourceDocument::new(
        "_project.md",
        parse_project("_project.md", &read_text(&project_path)?)?,
    );
    let entity_paths = markdown_files(&root.join("entities"), MAX_COLLECTION_FILES)?;
    let scenario_paths = markdown_files(
        &root.join("scenarios"),
        MAX_COLLECTION_FILES - entity_paths.len(),
    )?;
    let entities = entity_paths
        .into_iter()
        .map(|file| {
            let relative = relative_path(root, &file);
            Ok(SourceDocument::new(
                relative.clone(),
                parse_entity(relative, &read_text(&file)?)?,
            ))
        })
        .collect::<Result<_, DirectoryError>>()?;
    let scenarios = scenario_paths
        .into_iter()
        .map(|file| {
            let relative = relative_path(root, &file);
            Ok(SourceDocument::new(
                relative.clone(),
                parse_scenario(relative, &read_text(&file)?)?,
            ))
        })
        .collect::<Result<_, DirectoryError>>()?;
    Ok(ValidatedImport::new(project, entities, scenarios)?)
}

fn markdown_files(path: &Path, maximum: usize) -> Result<Vec<PathBuf>, DirectoryError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let entries =
        std::fs::read_dir(path).map_err(|error| directory_error::io("list", path, error))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| directory_error::io("read entry in", path, error))?;
        let file_type = entry
            .file_type()
            .map_err(|error| directory_error::io("inspect", entry.path(), error))?;
        if file_type.is_file() && entry.path().extension().is_some_and(|value| value == "md") {
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

fn read_text(path: &Path) -> Result<String, DirectoryError> {
    std::fs::read_to_string(path).map_err(|error| directory_error::io("read", path, error))
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("discovered below the supplied root")
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}
