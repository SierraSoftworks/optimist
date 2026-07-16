use std::collections::BTreeMap;

use indradb::MemoryDatastore;

use crate::{
    domain::{EntityId, ProjectDependenceModel, ProjectId, Scenario, ScenarioId, normalize_name},
    store::IndraDbRepository,
};

use super::{Project, ProjectError};
use crate::command::CommandResult;
use uuid::Uuid;

pub(super) struct ProjectEntry {
    pub(super) project: Project,
    pub(super) repository: IndraDbRepository<MemoryDatastore>,
    pub(super) results: BTreeMap<Uuid, CommandResult>,
    pub(super) next_scenario_id: Option<u64>,
    pub(super) scenarios: BTreeMap<ScenarioId, Scenario>,
    pub(super) dependence: Option<ProjectDependenceModel>,
}

/// Owns project metadata and one isolated graph repository per project.
///
/// The current catalog is process-local and uses IndraDB's memory datastore. It is
/// the lifecycle contract which a durable/lazy project manager will preserve when
/// RocksDB-backed handles are introduced.
///
/// ```
/// use optimist::project::ProjectCatalog;
///
/// let mut catalog = ProjectCatalog::new();
/// let project = catalog.create("Delivery".to_owned())?;
/// assert_eq!(catalog.get(&project.id)?, project);
/// # Ok::<(), optimist::project::ProjectError>(())
/// ```
pub struct ProjectCatalog {
    next_project_id: Option<u64>,
    pub(super) projects: BTreeMap<ProjectId, ProjectEntry>,
    names: BTreeMap<String, ProjectId>,
}

impl ProjectCatalog {
    /// Creates an empty catalog whose first allocated project ID is `A`.
    pub fn new() -> Self {
        Self {
            next_project_id: Some(0),
            projects: BTreeMap::new(),
            names: BTreeMap::new(),
        }
    }

    /// Creates a project and its isolated repository as one catalog operation.
    ///
    /// Names are unique after the same Unicode/case/whitespace normalization used
    /// for graph entities. No catalog entry is published if repository creation fails.
    pub fn create(&mut self, name: String) -> Result<Project, ProjectError> {
        let normalized_name = normalize_name(&name);
        if normalized_name.is_empty() {
            return Err(ProjectError::EmptyName);
        }
        if self.names.contains_key(&normalized_name) {
            return Err(ProjectError::DuplicateName(name));
        }
        let value = self
            .next_project_id
            .ok_or(ProjectError::IdentifierSpaceExhausted)?;
        let id = ProjectId::new(EntityId::new(value).to_string())
            .expect("entity IDs are valid project IDs");
        let project = Project {
            id: id.clone(),
            name,
            revision: 0,
        };
        let repository = IndraDbRepository::memory(id.clone())?;

        self.next_project_id = value.checked_add(1);
        self.names.insert(normalized_name, id.clone());
        self.projects.insert(
            id,
            ProjectEntry {
                project: project.clone(),
                repository,
                results: BTreeMap::new(),
                next_scenario_id: Some(0),
                scenarios: BTreeMap::new(),
                dependence: None,
            },
        );
        Ok(project)
    }

    /// Returns project metadata in deterministic project-ID order.
    ///
    /// Deterministic ordering keeps JSON/CLI output stable for agents and tests.
    pub fn list(&self) -> Vec<Project> {
        self.projects
            .values()
            .map(|entry| entry.project.clone())
            .collect()
    }

    /// Returns metadata for `id` without exposing or locking its graph repository.
    pub fn get(&self, id: &ProjectId) -> Result<Project, ProjectError> {
        self.projects
            .get(id)
            .map(|entry| entry.project.clone())
            .ok_or_else(|| ProjectError::NotFound(id.clone()))
    }

    /// Removes a project entry and drops its isolated in-memory graph.
    ///
    /// Durable implementations must replace this drop behavior with an explicit
    /// archive/delete policy so filesystem data is never silently discarded.
    pub fn delete(&mut self, id: &ProjectId) -> Result<Project, ProjectError> {
        let entry = self
            .projects
            .remove(id)
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        self.names.remove(&normalize_name(&entry.project.name));
        Ok(entry.project)
    }

    /// Borrows the isolated repository used for mutations within one project.
    ///
    /// Requiring a project ID at this boundary prevents entity and edge operations
    /// from accidentally crossing graph namespaces.
    pub fn repository_mut(
        &mut self,
        id: &ProjectId,
    ) -> Result<&mut IndraDbRepository<MemoryDatastore>, ProjectError> {
        self.projects
            .get_mut(id)
            .map(|entry| &mut entry.repository)
            .ok_or_else(|| ProjectError::NotFound(id.clone()))
    }
}

impl Default for ProjectCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::ProjectId;

    use super::{ProjectCatalog, ProjectError};

    #[test]
    fn allocates_project_local_graphs() {
        let mut catalog = ProjectCatalog::new();
        let first = catalog.create("Delivery".to_owned()).unwrap();
        let second = catalog.create("Security".to_owned()).unwrap();

        assert_eq!(first.id.as_str(), "A");
        assert_eq!(second.id.as_str(), "B");
        assert!(catalog.repository_mut(&first.id).is_ok());
        assert!(catalog.repository_mut(&second.id).is_ok());
    }

    #[test]
    fn project_names_are_case_insensitively_unique() {
        let mut catalog = ProjectCatalog::new();
        catalog.create("Delivery Health".to_owned()).unwrap();
        assert!(matches!(
            catalog.create(" delivery HEALTH ".to_owned()),
            Err(ProjectError::DuplicateName(_))
        ));
    }

    #[test]
    fn deleted_project_names_can_be_reused() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.delete(&project.id).unwrap();
        assert!(catalog.create("Delivery".to_owned()).is_ok());
        assert!(matches!(
            catalog.get(&ProjectId::new("A").unwrap()),
            Err(ProjectError::NotFound(_))
        ));
    }
}
