use std::collections::BTreeMap;

use super::{MarkdownError, ValidatedImport, render_entity, render_project, render_scenario};

/// Canonical project-relative Markdown files rendered from one validated revision.
///
/// The ordered map makes file order deterministic and contains no filesystem
/// metadata, so equal snapshots produce byte-identical contents on every export.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedSnapshot {
    files: BTreeMap<String, String>,
}

impl RenderedSnapshot {
    /// Renders a complete validated import using canonical paths and LF endings.
    pub fn from_import(import: &ValidatedImport) -> Result<Self, MarkdownError> {
        let mut files = BTreeMap::from([(
            "_project.md".to_owned(),
            render_project(&import.project.document)?,
        )]);
        for source in import.entities.values() {
            files.insert(
                source.document.canonical_path(),
                render_entity(&source.document)?,
            );
        }
        for source in import.scenarios.values() {
            files.insert(
                source.document.canonical_path(),
                render_scenario(&source.document)?,
            );
        }
        Ok(Self { files })
    }

    /// Iterates canonical project-relative paths and UTF-8 contents in path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &str)> {
        self.files
            .iter()
            .map(|(path, content)| (path.as_str(), content.as_str()))
    }
}
