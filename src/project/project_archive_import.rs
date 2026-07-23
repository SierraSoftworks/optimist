use std::collections::BTreeMap;

use indradb::MemoryDatastore;

use crate::store::{GraphRepository, IndraDbRepository};

use super::{ProjectArchive, ProjectCatalog, ProjectError, catalog::ProjectEntry};

impl ProjectCatalog {
    /// Restores a complete validated archive, publishing it only after all checks pass.
    pub fn import_archive(
        &mut self,
        archive: &ProjectArchive,
        replace: bool,
        yes: bool,
    ) -> Result<crate::project::Project, ProjectError> {
        let import = archive.validated_import()?;
        let project = import.project.document.project.clone();
        let existing = self.projects.contains_key(&project.id);
        if existing && (!replace || !yes) {
            return Err(if replace {
                ProjectError::ReplaceConfirmationRequired(project.id)
            } else {
                ProjectError::ImportProjectExists(project.id)
            });
        }
        self.publish_import(entry_from_import(&import)?)
    }
}

fn entry_from_import(
    import: &crate::project_yaml::ValidatedImport,
) -> Result<ProjectEntry, ProjectError> {
    let project = import.project.document.project.clone();
    let mut repository = IndraDbRepository::<MemoryDatastore>::memory(project.id.clone())?;
    for entity in import.entities.values() {
        repository.create_node(entity.document.node.clone())?;
    }
    for entity in import.entities.values() {
        for edge in &entity.document.outgoing_edges {
            repository.create_edge(edge.clone())?;
        }
    }
    let next_scenario_id = import
        .scenarios
        .keys()
        .next_back()
        .map_or(Some(0), |id| id.value().checked_add(1));
    let mut entry = ProjectEntry {
        project,
        description: import.project.document.description.clone(),
        // Project revision records archived document lineage. Graph revision starts
        // a new process-local analysis lineage because replay history resets.
        graph_revision: 0,
        repository,
        results: BTreeMap::new(),
        changes: BTreeMap::new(),
        change_history_start: import.project.document.project.revision,
        next_scenario_id,
        scenarios: import
            .scenarios
            .iter()
            .map(|(id, source)| (*id, source.document.scenario.clone()))
            .collect(),
        dependence: import.project.document.dependence.clone(),
    };
    // This entry is local and is dropped on any error below. The catalog only sees
    // it after every imported document validates against the fresh repository.
    if let Some(dependence) = entry.dependence.clone() {
        super::dependence_addresses::validate(&mut entry, &dependence)?;
    }
    Ok(entry)
}
