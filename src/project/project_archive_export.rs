use crate::{
    markdown::{
        EntityDocument, ProjectDocument, RenderedSnapshot, SCHEMA_VERSION, ScenarioDocument,
        SourceDocument, ValidatedImport,
    },
    store::GraphRepository,
};

use super::{
    ProjectArchive, ProjectArchiveSummary, ProjectCatalog, ProjectError, catalog::ProjectEntry,
};

impl ProjectCatalog {
    /// Exports one immutable project snapshot as canonical Markdown files in JSON.
    pub fn export_archive(
        &mut self,
        project_id: &crate::domain::ProjectId,
    ) -> Result<ProjectArchive, ProjectError> {
        let entry = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        let import = project_import(entry)?;
        let snapshot = RenderedSnapshot::from_import(&import)?;
        Ok(ProjectArchive {
            schema_version: SCHEMA_VERSION,
            project: import.project.document.project.clone(),
            files: snapshot
                .files()
                .map(|(path, contents)| (path.to_owned(), contents.to_owned()))
                .collect(),
            summary: summary(&import),
        })
    }
}

fn project_import(entry: &mut ProjectEntry) -> Result<ValidatedImport, ProjectError> {
    let revision = entry.project.revision;
    let nodes = entry.repository.list_nodes()?;
    let edges = entry.repository.list_edges()?;
    let entities = nodes
        .into_iter()
        .map(|node| {
            let document = EntityDocument {
                schema_version: SCHEMA_VERSION,
                base_project_revision: revision,
                outgoing_edges: edges
                    .iter()
                    .filter(|edge| edge.source == node.id)
                    .cloned()
                    .collect(),
                node,
            };
            SourceDocument::new(document.canonical_path(), document)
        })
        .collect();
    let scenarios = entry
        .scenarios
        .values()
        .cloned()
        .map(|scenario| {
            let document = ScenarioDocument {
                schema_version: SCHEMA_VERSION,
                base_project_revision: revision,
                scenario,
            };
            SourceDocument::new(document.canonical_path(), document)
        })
        .collect();
    let project = ProjectDocument {
        schema_version: SCHEMA_VERSION,
        project: entry.project.clone(),
        dependence: entry.dependence.clone(),
        formulas: entry.formulas.clone(),
        description: entry.description.clone(),
    };
    Ok(ValidatedImport::new(
        SourceDocument::new("_project.md", project),
        entities,
        scenarios,
    )?)
}

pub(super) fn summary(import: &ValidatedImport) -> ProjectArchiveSummary {
    ProjectArchiveSummary {
        entities: import.entities.len(),
        edges: import
            .entities
            .values()
            .map(|source| source.document.outgoing_edges.len())
            .sum(),
        scenarios: import.scenarios.len(),
    }
}
