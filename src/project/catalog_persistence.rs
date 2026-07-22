use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::command::{ChangeSet, CommandResult};
use crate::store::{GraphRepository, IndraDbRepository};
use indradb::MemoryDatastore;

use super::{ProjectArchive, ProjectCatalog, ProjectError, catalog::ProjectEntry};

const LEGACY_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
const PROJECT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const PROJECT_METADATA_SCHEMA_VERSION: u32 = 1;
const LEGACY_SNAPSHOT_FILE: &str = "catalog.json";
const PROJECTS_DIRECTORY: &str = "projects";
const PROJECT_METADATA_FILE: &str = "meta.json";
const PROJECT_SNAPSHOT_FILE: &str = "project.json";
const MAX_PROJECT_METADATA_BYTES: u64 = 64 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;

/// Failures while loading, validating, or atomically publishing a catalog snapshot.
#[derive(Debug, Error)]
pub enum CatalogPersistenceError {
    /// A snapshot path could not be read, created, synchronized, or renamed.
    #[error("could not access catalog snapshot {path}")]
    Io {
        /// Filesystem path involved in the failed operation.
        path: PathBuf,
        /// Underlying filesystem failure.
        #[source]
        source: std::io::Error,
    },
    /// The snapshot exceeds the bounded startup and publication size.
    #[error("catalog snapshot {path} exceeds the 512 MiB limit")]
    TooLarge {
        /// Snapshot path whose metadata or encoded content exceeded the limit.
        path: PathBuf,
    },
    /// Snapshot bytes are not valid JSON for the current envelope.
    #[error("catalog snapshot {path} is not valid JSON")]
    Json {
        /// Snapshot path containing invalid JSON.
        path: PathBuf,
        /// JSON decoding failure.
        #[source]
        source: serde_json::Error,
    },
    /// The snapshot declares an unsupported forward or legacy schema version.
    #[error("catalog snapshot schema {0} is unsupported")]
    UnsupportedSchema(u32),
    /// A pending command uses a journal schema this server cannot safely replay.
    #[error("command journal schema {0} is unsupported")]
    UnsupportedJournalSchema(u32),
    /// A known older snapshot could not be transformed into the next schema.
    #[error("could not migrate catalog snapshot schema {version}: {reason}")]
    Migration {
        /// Schema version being migrated.
        version: u32,
        /// Structural problem which made the migration unsafe.
        reason: String,
    },
    /// One embedded canonical project archive or allocator failed validation.
    #[error("catalog snapshot contains invalid project state")]
    Project(#[from] ProjectError),
}

pub(crate) struct CatalogStore {
    pub(super) root: PathBuf,
    pub(super) journal_lock: Mutex<()>,
    snapshot_lock: Mutex<()>,
}

impl CatalogStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            journal_lock: Mutex::new(()),
            snapshot_lock: Mutex::new(()),
        }
    }

    pub(crate) fn load(&self) -> Result<ProjectCatalog, CatalogPersistenceError> {
        let legacy = self.root.join(LEGACY_SNAPSHOT_FILE);
        let mut catalog = if legacy.exists() {
            let mut catalog = self.load_file(&legacy, false)?;
            self.save(&mut catalog)?;
            fs::remove_file(&legacy).map_err(|source| io_error(legacy, source))?;
            catalog
        } else {
            self.load_project_directories()?
        };
        let _ = self.recover_pending_mutations(&mut catalog)?;
        Ok(catalog)
    }

    pub(crate) fn list_project_metadata(
        &self,
    ) -> Result<Vec<super::Project>, CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => return Err(io_error(directory, source)),
        };
        let mut projects = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error(directory.clone(), source))?;
            if !entry
                .file_type()
                .map_err(|source| io_error(entry.path(), source))?
                .is_dir()
            {
                continue;
            }
            let metadata_path = entry.path().join(PROJECT_METADATA_FILE);
            let metadata_bytes = read_bounded_to(&metadata_path, MAX_PROJECT_METADATA_BYTES)?;
            let metadata: ProjectMetadataDocument = serde_json::from_slice(&metadata_bytes)
                .map_err(|source| CatalogPersistenceError::Json {
                    path: metadata_path.clone(),
                    source,
                })?;
            if metadata.schema_version != PROJECT_METADATA_SCHEMA_VERSION {
                return Err(CatalogPersistenceError::UnsupportedSchema(
                    metadata.schema_version,
                ));
            }
            if !metadata.deleted
                && let Some(project) = metadata.project
            {
                projects.push(project);
            }
        }
        projects.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(projects)
    }

    pub(super) fn load_file(
        &self,
        path: &Path,
        rewrite_migration: bool,
    ) -> Result<ProjectCatalog, CatalogPersistenceError> {
        let path = path.to_path_buf();
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProjectCatalog::new());
            }
            Err(source) => return Err(io_error(path, source)),
        };
        if metadata.len() > MAX_SNAPSHOT_BYTES {
            return Err(CatalogPersistenceError::TooLarge { path });
        }
        let bytes = fs::read(&path).map_err(|source| io_error(path.clone(), source))?;
        let mut document: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|source| CatalogPersistenceError::Json {
                path: path.clone(),
                source,
            })?;
        migrate_legacy(&mut document)?;
        let snapshot: LegacyCatalogSnapshot =
            serde_json::from_value(document).map_err(|source| CatalogPersistenceError::Json {
                path: path.clone(),
                source,
            })?;
        let mut catalog = ProjectCatalog::from_legacy_snapshot(snapshot)?;
        if rewrite_migration {
            self.save(&mut catalog)?;
        }
        Ok(catalog)
    }

    pub(crate) fn save(&self, catalog: &mut ProjectCatalog) -> Result<(), CatalogPersistenceError> {
        let _guard = self.snapshot_lock.lock().expect("snapshot lock poisoned");
        self.save_unlocked(catalog)
    }

    pub(crate) fn save_if_current(
        &self,
        catalog: &mut ProjectCatalog,
        projects: &BTreeSet<crate::domain::ProjectId>,
        generation: &AtomicU64,
        expected_generation: u64,
    ) -> Result<bool, CatalogPersistenceError> {
        let _guard = self.snapshot_lock.lock().expect("snapshot lock poisoned");
        if generation.load(Ordering::Acquire) != expected_generation {
            return Ok(false);
        }
        self.save_projects_unlocked(catalog, projects)?;
        Ok(true)
    }

    fn save_unlocked(&self, catalog: &mut ProjectCatalog) -> Result<(), CatalogPersistenceError> {
        fs::create_dir_all(&self.root).map_err(|source| io_error(self.root.clone(), source))?;
        let projects = catalog.projects.keys().cloned().collect::<BTreeSet<_>>();
        self.save_projects_unlocked(catalog, &projects)?;
        self.remove_orphan_projects(&projects)?;
        if let Some(last_allocated) = catalog
            .next_project_id
            .and_then(|value| value.checked_sub(1))
        {
            let project = crate::domain::ProjectId::new(
                crate::domain::EntityId::new(last_allocated).to_string(),
            )
            .expect("generated entity IDs are valid project IDs");
            if !projects.contains(&project) {
                self.write_project_tombstone(&project)?;
            }
        }
        Ok(())
    }

    fn save_projects_unlocked(
        &self,
        catalog: &mut ProjectCatalog,
        projects: &BTreeSet<crate::domain::ProjectId>,
    ) -> Result<(), CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY);
        fs::create_dir_all(&directory).map_err(|source| io_error(directory.clone(), source))?;
        for project in projects {
            let project_directory = project_directory(&directory, project);
            fs::create_dir_all(&project_directory)
                .map_err(|source| io_error(project_directory.clone(), source))?;
            let document = ProjectSnapshotDocument {
                schema_version: PROJECT_SNAPSHOT_SCHEMA_VERSION,
                project: catalog.persisted_project(project)?,
            };
            let bytes = serde_json::to_vec(&document).expect("project snapshots serialize");
            if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
                return Err(CatalogPersistenceError::TooLarge {
                    path: project_directory.join(PROJECT_SNAPSHOT_FILE),
                });
            }
            atomic_write(&project_directory, PROJECT_SNAPSHOT_FILE, &bytes)?;
            let metadata = ProjectMetadataDocument {
                schema_version: PROJECT_METADATA_SCHEMA_VERSION,
                project: Some(document.project.archive.project.clone()),
                deleted: false,
            };
            atomic_write(
                &project_directory,
                PROJECT_METADATA_FILE,
                &serde_json::to_vec(&metadata).expect("project metadata serializes"),
            )?;
        }
        Ok(())
    }

    fn remove_orphan_projects(
        &self,
        projects: &BTreeSet<crate::domain::ProjectId>,
    ) -> Result<(), CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY);
        let entries =
            fs::read_dir(&directory).map_err(|source| io_error(directory.clone(), source))?;
        for entry in entries {
            let entry = entry.map_err(|source| io_error(directory.clone(), source))?;
            let path = entry.path();
            if !entry
                .file_type()
                .map_err(|source| io_error(path.clone(), source))?
                .is_dir()
                || entry.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            let retained = crate::domain::ProjectId::new(entry.file_name().to_string_lossy())
                .ok()
                .is_some_and(|project| projects.contains(&project));
            if !retained {
                let Ok(project) =
                    crate::domain::ProjectId::new(entry.file_name().to_string_lossy())
                else {
                    continue;
                };
                for name in [PROJECT_SNAPSHOT_FILE, "journal.json"] {
                    let file = path.join(name);
                    match fs::remove_file(&file) {
                        Ok(()) => {}
                        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                        Err(source) => return Err(io_error(file, source)),
                    }
                }
                self.write_project_tombstone(&project)?;
            }
        }
        Ok(())
    }

    fn write_project_tombstone(
        &self,
        project: &crate::domain::ProjectId,
    ) -> Result<(), CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY).join(project.as_str());
        fs::create_dir_all(&directory).map_err(|source| io_error(directory.clone(), source))?;
        let metadata = ProjectMetadataDocument {
            schema_version: PROJECT_METADATA_SCHEMA_VERSION,
            project: None,
            deleted: true,
        };
        atomic_write(
            &directory,
            PROJECT_METADATA_FILE,
            &serde_json::to_vec(&metadata).expect("project tombstones serialize"),
        )
    }

    fn load_project_directories(&self) -> Result<ProjectCatalog, CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProjectCatalog::new());
            }
            Err(source) => return Err(io_error(directory, source)),
        };
        let mut projects = Vec::new();
        let mut allocated_ids = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error(directory.clone(), source))?;
            let path = entry.path();
            if !entry
                .file_type()
                .map_err(|source| io_error(path.clone(), source))?
                .is_dir()
                || entry.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            let id = crate::domain::ProjectId::new(entry.file_name().to_string_lossy()).map_err(
                |error| {
                    CatalogPersistenceError::Project(ProjectError::InvalidArchivePath(format!(
                        "invalid project directory: {error}"
                    )))
                },
            )?;
            allocated_ids.push(id.clone());
            let metadata_path = path.join(PROJECT_METADATA_FILE);
            let metadata: ProjectMetadataDocument = serde_json::from_slice(&read_bounded_to(
                &metadata_path,
                MAX_PROJECT_METADATA_BYTES,
            )?)
            .map_err(|source| CatalogPersistenceError::Json {
                path: metadata_path.clone(),
                source,
            })?;
            if metadata.schema_version != PROJECT_METADATA_SCHEMA_VERSION {
                return Err(CatalogPersistenceError::Project(
                    ProjectError::InvalidArchivePath(format!(
                        "invalid project metadata {}",
                        metadata_path.display()
                    )),
                ));
            }
            if metadata.deleted {
                if metadata.project.is_some() {
                    return Err(CatalogPersistenceError::Project(
                        ProjectError::InvalidArchivePath(format!(
                            "deleted project metadata contains a project {}",
                            metadata_path.display()
                        )),
                    ));
                }
                continue;
            }
            let metadata_project = metadata.project.ok_or_else(|| {
                CatalogPersistenceError::Project(ProjectError::InvalidArchivePath(format!(
                    "active project metadata is empty {}",
                    metadata_path.display()
                )))
            })?;
            if metadata_project.id != id {
                return Err(CatalogPersistenceError::Project(
                    ProjectError::InvalidArchivePath(format!(
                        "project metadata ID does not match {}",
                        metadata_path.display()
                    )),
                ));
            }
            let snapshot_path = path.join(PROJECT_SNAPSHOT_FILE);
            let document: ProjectSnapshotDocument =
                serde_json::from_slice(&read_bounded(&snapshot_path)?).map_err(|source| {
                    CatalogPersistenceError::Json {
                        path: snapshot_path.clone(),
                        source,
                    }
                })?;
            let snapshot_project = &document.project.archive.project;
            if document.schema_version != PROJECT_SNAPSHOT_SCHEMA_VERSION
                || snapshot_project.id != metadata_project.id
                || snapshot_project.name != metadata_project.name
            {
                return Err(CatalogPersistenceError::Project(
                    ProjectError::InvalidArchivePath(format!(
                        "project metadata does not match {}",
                        snapshot_path.display()
                    )),
                ));
            }
            if snapshot_project.revision != metadata_project.revision {
                let repaired = ProjectMetadataDocument {
                    schema_version: PROJECT_METADATA_SCHEMA_VERSION,
                    project: Some(snapshot_project.clone()),
                    deleted: false,
                };
                atomic_write(
                    &path,
                    PROJECT_METADATA_FILE,
                    &serde_json::to_vec(&repaired).expect("project metadata serializes"),
                )?;
            }
            projects.push(document.project);
        }
        let next_project_id = allocated_ids
            .iter()
            .filter_map(|project| project.as_str().parse::<crate::domain::EntityId>().ok())
            .map(crate::domain::EntityId::value)
            .max()
            .map_or(Some(0), |value| value.checked_add(1));
        ProjectCatalog::from_persisted_projects(next_project_id, projects)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProjectMetadataDocument {
    schema_version: u32,
    project: Option<super::Project>,
    #[serde(default)]
    deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProjectSnapshotDocument {
    schema_version: u32,
    project: PersistedProject,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LegacyCatalogSnapshot {
    schema_version: u32,
    next_project_id: Option<u64>,
    projects: Vec<PersistedProject>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedProject {
    archive: ProjectArchive,
    graph_revision: u64,
    next_entity_id: Option<u64>,
    next_scenario_id: Option<u64>,
    change_history_start: u64,
    changes: Vec<ChangeSet>,
}

impl ProjectCatalog {
    pub(crate) fn transaction_clone(&mut self) -> Result<Self, CatalogPersistenceError> {
        let mut candidate = Self::new();
        candidate.next_project_id = self.next_project_id;
        for entry in self.projects.values() {
            candidate.publish_import(clone_entry(entry)?)?;
        }
        Ok(candidate)
    }

    pub(crate) fn transaction_clone_projects(
        &self,
        projects: &BTreeSet<crate::domain::ProjectId>,
    ) -> Result<Self, CatalogPersistenceError> {
        let mut candidate = Self::new();
        candidate.next_project_id = self.next_project_id;
        for project in projects {
            let entry = self
                .projects
                .get(project)
                .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
            candidate.publish_import(clone_entry(entry)?)?;
        }
        Ok(candidate)
    }

    pub(crate) fn project_transaction_clone(
        &self,
        project: &crate::domain::ProjectId,
    ) -> Result<Self, CatalogPersistenceError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        let mut candidate = Self::new();
        candidate.next_project_id = self.next_project_id;
        candidate.publish_import(clone_entry(entry)?)?;
        Ok(candidate)
    }

    pub(crate) fn publish_project_transaction(
        &mut self,
        project: &crate::domain::ProjectId,
        mut candidate: Self,
    ) -> Result<(), ProjectError> {
        let entry = candidate
            .projects
            .remove(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        self.publish_import(entry)?;
        Ok(())
    }

    fn persisted_project(
        &mut self,
        id: &crate::domain::ProjectId,
    ) -> Result<PersistedProject, ProjectError> {
        let archive = self.export_archive(id)?;
        let entry = self
            .projects
            .get(id)
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        Ok(PersistedProject {
            archive,
            graph_revision: entry.graph_revision,
            next_entity_id: entry.repository.next_entity_id_counter(),
            next_scenario_id: entry.next_scenario_id,
            change_history_start: entry.change_history_start,
            changes: entry.changes.values().cloned().collect(),
        })
    }

    fn from_legacy_snapshot(
        snapshot: LegacyCatalogSnapshot,
    ) -> Result<Self, CatalogPersistenceError> {
        if snapshot.schema_version != LEGACY_SNAPSHOT_SCHEMA_VERSION {
            return Err(CatalogPersistenceError::UnsupportedSchema(
                snapshot.schema_version,
            ));
        }
        Self::from_persisted_projects(snapshot.next_project_id, snapshot.projects)
    }

    fn from_persisted_projects(
        next_project_id: Option<u64>,
        projects: Vec<PersistedProject>,
    ) -> Result<Self, CatalogPersistenceError> {
        let mut catalog = Self::new();
        for persisted in projects {
            let id = persisted.archive.project.id.clone();
            catalog.import_archive(&persisted.archive, false, false)?;
            let entry = catalog
                .projects
                .get_mut(&id)
                .expect("import published restored project");
            validate_allocator(
                "scenario",
                entry.next_scenario_id,
                persisted.next_scenario_id,
            )?;
            entry.graph_revision = persisted.graph_revision;
            entry.next_scenario_id = persisted.next_scenario_id;
            restore_changes(entry, persisted.change_history_start, persisted.changes)?;
            entry
                .repository
                .restore_next_entity_id_counter(persisted.next_entity_id)
                .map_err(ProjectError::from)?;
        }
        catalog.next_project_id =
            next_project_id.or_else(|| catalog.next_project_id_from_projects());
        Ok(catalog)
    }

    fn next_project_id_from_projects(&self) -> Option<u64> {
        self.projects
            .keys()
            .filter_map(|project| project.as_str().parse::<crate::domain::EntityId>().ok())
            .map(crate::domain::EntityId::value)
            .max()
            .map_or(Some(0), |value| value.checked_add(1))
    }
}

fn clone_entry(entry: &ProjectEntry) -> Result<ProjectEntry, ProjectError> {
    let mut repository = IndraDbRepository::<MemoryDatastore>::memory(entry.project.id.clone())?;
    for node in entry.repository.list_nodes()? {
        repository.create_node(node)?;
    }
    for edge in entry.repository.list_edges()? {
        repository.create_edge(edge)?;
    }
    repository.restore_next_entity_id_counter(entry.repository.next_entity_id_counter())?;
    Ok(ProjectEntry {
        project: entry.project.clone(),
        description: entry.description.clone(),
        graph_revision: entry.graph_revision,
        repository,
        results: entry.results.clone(),
        changes: entry.changes.clone(),
        change_history_start: entry.change_history_start,
        next_scenario_id: entry.next_scenario_id,
        scenarios: entry.scenarios.clone(),
        dependence: entry.dependence.clone(),
        formulas: entry.formulas.clone(),
    })
}

fn schema_version(document: &serde_json::Value) -> Result<u32, CatalogPersistenceError> {
    document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| migration_error(0, "schema_version must be a u32"))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, CatalogPersistenceError> {
    read_bounded_to(path, MAX_SNAPSHOT_BYTES)
}

fn read_bounded_to(path: &Path, maximum: u64) -> Result<Vec<u8>, CatalogPersistenceError> {
    let metadata = fs::metadata(path).map_err(|source| io_error(path.to_path_buf(), source))?;
    if metadata.len() > maximum {
        return Err(CatalogPersistenceError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    fs::read(path).map_err(|source| io_error(path.to_path_buf(), source))
}

fn project_directory(directory: &Path, project: &crate::domain::ProjectId) -> PathBuf {
    directory.join(project.as_str())
}

fn migrate_legacy(document: &mut serde_json::Value) -> Result<(), CatalogPersistenceError> {
    let version = schema_version(document)?;
    match version {
        LEGACY_SNAPSHOT_SCHEMA_VERSION => Ok(()),
        1 => {
            migrate_v1_to_v2(document)?;
            Ok(())
        }
        unsupported => Err(CatalogPersistenceError::UnsupportedSchema(unsupported)),
    }
}

fn migrate_v1_to_v2(document: &mut serde_json::Value) -> Result<(), CatalogPersistenceError> {
    let projects = document
        .get_mut("projects")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| migration_error(1, "projects must be an array"))?;
    for project in projects {
        let object = project
            .as_object_mut()
            .ok_or_else(|| migration_error(1, "each project must be an object"))?;
        let project_revision = object
            .get("archive")
            .and_then(|archive| archive.get("project"))
            .and_then(|project| project.get("revision"))
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| migration_error(1, "archive project revision must be a u64"))?;
        let history_start_missing = object
            .get("change_history_start")
            .is_none_or(serde_json::Value::is_null);
        let changes = object
            .entry("changes")
            .or_insert_with(|| serde_json::Value::Array(vec![]))
            .as_array()
            .ok_or_else(|| migration_error(1, "changes must be an array"))?;
        if history_start_missing {
            let history_start = changes
                .first()
                .and_then(|change| change.get("base_revision"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(project_revision);
            object.insert(
                "change_history_start".to_owned(),
                serde_json::Value::from(history_start),
            );
        }
    }
    document["schema_version"] = serde_json::Value::from(LEGACY_SNAPSHOT_SCHEMA_VERSION);
    Ok(())
}

fn migration_error(version: u32, reason: &str) -> CatalogPersistenceError {
    CatalogPersistenceError::Migration {
        version,
        reason: reason.to_owned(),
    }
}

fn restore_changes(
    entry: &mut super::catalog::ProjectEntry,
    history_start: u64,
    changes: Vec<ChangeSet>,
) -> Result<(), CatalogPersistenceError> {
    if history_start > entry.project.revision {
        return Err(invalid_history("history start exceeds project revision"));
    }
    super::command_batch_history::validate_persisted_batches(&changes).map_err(invalid_history)?;
    let mut expected = history_start.checked_add(1);
    for change in changes {
        if Some(change.project_revision) != expected
            || change.base_revision.checked_add(1) != Some(change.project_revision)
            || change.project_revision > entry.project.revision
            || entry
                .changes
                .insert(change.project_revision, change.clone())
                .is_some()
            || entry
                .results
                .insert(
                    change.request_id,
                    CommandResult {
                        request_id: change.request_id,
                        project_revision: change.project_revision,
                        outcome: change.outcome,
                    },
                )
                .is_some()
        {
            return Err(invalid_history("changes are not unique and contiguous"));
        }
        expected = change.project_revision.checked_add(1);
    }
    let final_revision = expected
        .and_then(|value| value.checked_sub(1))
        .unwrap_or(history_start);
    if final_revision != entry.project.revision {
        return Err(invalid_history("changes do not reach the project revision"));
    }
    entry.change_history_start = history_start;
    Ok(())
}

fn invalid_history(message: &str) -> CatalogPersistenceError {
    CatalogPersistenceError::Project(ProjectError::InvalidArchivePath(format!(
        "invalid persisted change history: {message}"
    )))
}

fn validate_allocator(
    kind: &str,
    minimum: Option<u64>,
    persisted: Option<u64>,
) -> Result<(), CatalogPersistenceError> {
    if let (Some(minimum), Some(persisted)) = (minimum, persisted)
        && persisted < minimum
    {
        return Err(CatalogPersistenceError::Project(
            ProjectError::InvalidArchivePath(format!(
                "persisted next {kind} ID {persisted} precedes required value {minimum}"
            )),
        ));
    }
    if minimum.is_none() && persisted.is_some() {
        return Err(CatalogPersistenceError::Project(
            ProjectError::InvalidArchivePath(format!(
                "exhausted {kind} allocator cannot be restored"
            )),
        ));
    }
    Ok(())
}

pub(super) fn atomic_write(
    root: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<(), CatalogPersistenceError> {
    let target = root.join(name);
    let temporary = root.join(format!(".{name}.tmp"));
    let mut file =
        fs::File::create(&temporary).map_err(|source| io_error(temporary.clone(), source))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(temporary.clone(), source))?;
    fs::rename(&temporary, &target).map_err(|source| io_error(target, source))?;
    fs::File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(root.to_path_buf(), source))?;
    Ok(())
}

fn io_error(path: PathBuf, source: std::io::Error) -> CatalogPersistenceError {
    CatalogPersistenceError::Io { path, source }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        command::{
            CommandBatchRequest, CommandRequest, CreateNode, CreateScenario, DeleteNode,
            DeleteScenario, GraphCommand,
        },
        domain::{EntityId, Factor, MonteCarloConfig, NodePayload, ScenarioDraft, ScenarioId},
        server::router_with_persistent_catalog,
    };

    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("optimist-catalog-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn factor(name: &str) -> CreateNode {
        CreateNode {
            name: name.to_owned(),
            title: name.to_owned(),
            payload: NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: false,
                evidence: vec![],
            }),
        }
    }

    fn node_request() -> CommandRequest {
        CommandRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            command: GraphCommand::CreateNode(factor("flow")),
        }
    }

    fn write_legacy_snapshot(root: &Path, catalog: &mut ProjectCatalog) -> PathBuf {
        let ids = catalog.projects.keys().cloned().collect::<Vec<_>>();
        let projects = ids
            .iter()
            .map(|project| catalog.persisted_project(project).unwrap())
            .collect();
        let snapshot = LegacyCatalogSnapshot {
            schema_version: LEGACY_SNAPSHOT_SCHEMA_VERSION,
            next_project_id: catalog.next_project_id,
            projects,
        };
        let path = root.join(LEGACY_SNAPSHOT_FILE);
        fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
        path
    }

    fn scenario(name: &str) -> ScenarioDraft {
        ScenarioDraft {
            name: name.to_owned(),
            title: name.to_owned(),
            rationale: String::new(),
            objectives: vec![],
            planning_horizon: 1,
            budgets: vec![],
            candidate_interventions: vec![],
            monte_carlo: MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
            scalar_preferences: None,
        }
    }

    async fn wait_for_persistence(app: &axum::Router, expected: &str) -> serde_json::Value {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let response = app
                    .clone()
                    .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                let body = to_bytes(response.into_body(), 4096).await.unwrap();
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                if value["persistence"]["state"] == expected {
                    return value;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("persistence reaches expected state")
    }

    #[test]
    fn restart_restores_contents_and_never_reuses_deleted_ids() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let first = catalog.create("Delivery".to_owned()).unwrap();
        let deleted_project = catalog.create("Temporary".to_owned()).unwrap();
        catalog.delete(&deleted_project.id).unwrap();
        let first_request = CommandRequest {
            request_id: Uuid::nil(),
            expected_revision: 0,
            command: GraphCommand::CreateNode(factor("first")),
        };
        let first_result = catalog.execute(&first.id, first_request.clone()).unwrap();
        catalog
            .execute(
                &first.id,
                CommandRequest::new(1, GraphCommand::CreateNode(factor("deleted"))),
            )
            .unwrap();
        catalog
            .execute(
                &first.id,
                CommandRequest::new(
                    2,
                    GraphCommand::DeleteNode(DeleteNode {
                        id: EntityId::new(1),
                    }),
                ),
            )
            .unwrap();
        catalog
            .execute(
                &first.id,
                CommandRequest::new(
                    3,
                    GraphCommand::CreateScenario(CreateScenario {
                        scenario: scenario("deleted"),
                    }),
                ),
            )
            .unwrap();
        catalog
            .execute(
                &first.id,
                CommandRequest::new(
                    4,
                    GraphCommand::DeleteScenario(DeleteScenario {
                        id: ScenarioId::new(0),
                        expected_revision: 0,
                    }),
                ),
            )
            .unwrap();
        store.save(&mut catalog).unwrap();

        let mut restored = store.load().unwrap();
        assert_eq!(restored.get(&first.id).unwrap().revision, 5);
        assert_eq!(restored.list_nodes(&first.id).unwrap().len(), 1);
        let replay = restored.replay_changes(&first.id, 0).unwrap();
        assert_eq!(replay.current_revision, 5);
        assert_eq!(
            replay
                .changes
                .iter()
                .map(|change| change.project_revision)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(
            restored.execute(&first.id, first_request).unwrap(),
            first_result
        );
        assert_eq!(restored.get(&first.id).unwrap().revision, 5);
        assert_eq!(restored.list_nodes(&first.id).unwrap().len(), 1);
        let project = restored.create("Next".to_owned()).unwrap();
        assert_eq!(project.id.as_str(), "C");
        let node = restored
            .execute(
                &first.id,
                CommandRequest::new(5, GraphCommand::CreateNode(factor("next"))),
            )
            .unwrap();
        let crate::command::CommandOutcome::NodeCreated(node) = node.outcome else {
            panic!("expected node creation")
        };
        assert_eq!(node.id, EntityId::new(2));
        let scenario = restored
            .execute(
                &first.id,
                CommandRequest::new(
                    6,
                    GraphCommand::CreateScenario(CreateScenario {
                        scenario: scenario("next"),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::ScenarioCreated(scenario) = scenario.outcome else {
            panic!("expected scenario creation")
        };
        assert_eq!(scenario.id, ScenarioId::new(1));
    }

    #[test]
    fn rejects_corrupt_and_unsupported_snapshots() {
        let fixture = Fixture::new();
        let path = fixture.root.join(LEGACY_SNAPSHOT_FILE);
        fs::write(&path, b"not json").unwrap();
        assert!(matches!(
            CatalogStore::new(fixture.root.clone()).load(),
            Err(CatalogPersistenceError::Json { .. })
        ));
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 99,
                "next_project_id": 0,
                "projects": []
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(matches!(
            CatalogStore::new(fixture.root.clone()).load(),
            Err(CatalogPersistenceError::UnsupportedSchema(99))
        ));
    }

    #[test]
    fn rejects_corrupt_batch_lineage_without_rewriting_the_snapshot() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute_batch(
                &project.id,
                CommandBatchRequest {
                    request_id: Uuid::new_v4(),
                    expected_revision: 0,
                    commands: vec![GraphCommand::CreateNode(factor("flow"))],
                },
                None,
            )
            .unwrap();
        let path = write_legacy_snapshot(&fixture.root, &mut catalog);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["projects"][0]["changes"][0]["request_id"] =
            serde_json::Value::String(Uuid::nil().to_string());
        let corrupted = serde_json::to_vec(&value).unwrap();
        fs::write(&path, &corrupted).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogPersistenceError::Project(_))
        ));
        assert_eq!(fs::read(path).unwrap(), corrupted);
    }

    #[test]
    fn migrates_v1_replay_and_idempotency_then_rewrites_v2() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let request = CommandRequest {
            request_id: Uuid::nil(),
            expected_revision: 0,
            command: GraphCommand::CreateNode(factor("flow")),
        };
        let first = catalog.execute(&project.id, request.clone()).unwrap();
        let path = write_legacy_snapshot(&fixture.root, &mut catalog);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(1);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut migrated = store.load().unwrap();
        assert_eq!(
            migrated
                .replay_changes(&project.id, 0)
                .unwrap()
                .changes
                .len(),
            1
        );
        assert_eq!(migrated.execute(&project.id, request).unwrap(), first);
        assert!(!path.exists());
        let rewritten: ProjectSnapshotDocument = serde_json::from_slice(
            &fs::read(
                fixture
                    .root
                    .join(PROJECTS_DIRECTORY)
                    .join(project.id.as_str())
                    .join(PROJECT_SNAPSHOT_FILE),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rewritten.schema_version, PROJECT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(rewritten.project.change_history_start, 0);
        assert_eq!(rewritten.project.changes.len(), 1);
    }

    #[test]
    fn migrates_pre_replay_v1_to_a_safe_history_floor() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("flow"))),
            )
            .unwrap();
        let path = write_legacy_snapshot(&fixture.root, &mut catalog);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(1);
        value["projects"][0]
            .as_object_mut()
            .unwrap()
            .remove("change_history_start");
        value["projects"][0]
            .as_object_mut()
            .unwrap()
            .remove("changes");
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut migrated = store.load().unwrap();
        assert!(matches!(
            migrated.replay_changes(&project.id, 0),
            Err(ProjectError::ChangeHistoryGap {
                available_after: 1,
                ..
            })
        ));
        assert!(
            migrated
                .replay_changes_with_snapshot(&project.id, 0)
                .unwrap()
                .snapshot
                .is_some()
        );
        assert!(!path.exists());
        let rewritten: ProjectSnapshotDocument = serde_json::from_slice(
            &fs::read(
                fixture
                    .root
                    .join(PROJECTS_DIRECTORY)
                    .join(project.id.as_str())
                    .join(PROJECT_SNAPSHOT_FILE),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(rewritten.project.change_history_start, 1);
        assert!(rewritten.project.changes.is_empty());
    }

    #[test]
    fn failed_v1_integrity_validation_does_not_rewrite_snapshot() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("flow"))),
            )
            .unwrap();
        let path = write_legacy_snapshot(&fixture.root, &mut catalog);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::Value::from(1);
        value["projects"][0]["change_history_start"] = serde_json::Value::from(0);
        value["projects"][0]["changes"] = serde_json::json!([]);
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        assert!(store.load().is_err());
        let unchanged: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(unchanged["schema_version"], 1);
    }

    #[test]
    fn imported_archives_report_their_unavailable_history_floor() {
        let mut source = ProjectCatalog::new();
        let project = source.create("Delivery".to_owned()).unwrap();
        source
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("flow"))),
            )
            .unwrap();
        let archive = source.export_archive(&project.id).unwrap();
        let mut restored = ProjectCatalog::new();
        restored.import_archive(&archive, false, false).unwrap();
        assert!(matches!(
            restored.replay_changes(&project.id, 0),
            Err(ProjectError::ChangeHistoryGap {
                requested: 0,
                available_after: 1
            })
        ));
        let replay = restored.replay_changes(&project.id, 1).unwrap();
        assert_eq!(replay.current_revision, 1);
        assert!(replay.changes.is_empty());
        assert!(replay.snapshot.is_none());

        let fallback = restored
            .replay_changes_with_snapshot(&project.id, 0)
            .unwrap();
        let snapshot = fallback.snapshot.expect("history gap returns snapshot");
        assert_eq!(snapshot.revision, fallback.current_revision);
        assert_eq!(snapshot.archive.project.revision, fallback.current_revision);
        assert_eq!(snapshot.archive.project.id, project.id);
        assert!(fallback.changes.is_empty());
    }

    #[tokio::test]
    async fn failed_publication_does_not_expose_the_mutation() {
        let fixture = Fixture::new();
        let projects_path = fixture.root.join(PROJECTS_DIRECTORY);
        fs::write(&projects_path, b"blocked").unwrap();
        let app = router_with_persistent_catalog(
            ProjectCatalog::new(),
            CatalogStore::new(fixture.root.clone()),
        );
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Delivery"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["code"], "catalog_persistence_failure");
        let list = app
            .oneshot(
                Request::get("/api/v1/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(list.into_body(), 4096).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn durable_transactions_preserve_process_local_retry_and_replay_state() {
        let fixture = Fixture::new();
        let app = router_with_persistent_catalog(
            ProjectCatalog::new(),
            CatalogStore::new(fixture.root.clone()),
        );
        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"Delivery"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let request = Request::post("/api/v1/projects/A/commands")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(&CommandRequest {
                    request_id: Uuid::nil(),
                    expected_revision: 0,
                    command: GraphCommand::CreateNode(factor("flow")),
                })
                .unwrap(),
            ))
            .unwrap();
        let first = app.clone().oneshot(request).await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = to_bytes(first.into_body(), 16 * 1024).await.unwrap();
        let retry = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&CommandRequest {
                            request_id: Uuid::nil(),
                            expected_revision: 0,
                            command: GraphCommand::CreateNode(factor("flow")),
                        })
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let retry_body = to_bytes(retry.into_body(), 16 * 1024).await.unwrap();
        assert_eq!(first_body, retry_body);
        let replay = app
            .clone()
            .oneshot(
                Request::get("/api/v1/projects/A/changes?after=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(replay.into_body(), 16 * 1024).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["current_revision"], 1);
        assert_eq!(value["changes"].as_array().unwrap().len(), 1);
        wait_for_persistence(&app, "idle").await;
        assert!(
            !fixture
                .root
                .join(PROJECTS_DIRECTORY)
                .join("A")
                .join("journal.json")
                .exists()
        );

        let restarted_store = CatalogStore::new(fixture.root.clone());
        let rejected =
            router_with_persistent_catalog(restarted_store.load().unwrap(), restarted_store)
                .oneshot(
                    Request::post("/api/v1/projects/A/commands")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::to_vec(&CommandRequest {
                                request_id: Uuid::new_v4(),
                                expected_revision: 0,
                                command: GraphCommand::CreateNode(factor("stale")),
                            })
                            .unwrap(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
        assert_eq!(rejected.status(), StatusCode::CONFLICT);
        assert!(
            !fixture
                .root
                .join(PROJECTS_DIRECTORY)
                .join("A")
                .join("journal.json")
                .exists()
        );
    }

    #[tokio::test]
    async fn command_acknowledgement_precedes_snapshot_compaction() {
        let fixture = Fixture::new();
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let store = CatalogStore::new(fixture.root.clone());
        store.save(&mut catalog).unwrap();
        let project_directory = fixture.root.join(PROJECTS_DIRECTORY).join("A");
        let snapshot_path = project_directory.join(PROJECT_SNAPSHOT_FILE);
        let journal_path = project_directory.join("journal.json");
        let snapshot_before = fs::read(&snapshot_path).unwrap();
        assert!(project_directory.join(PROJECT_METADATA_FILE).exists());
        let app = router_with_persistent_catalog(catalog, store);

        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&node_request()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(journal_path.exists());
        assert_eq!(fs::read(&snapshot_path).unwrap(), snapshot_before);
        let health = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = to_bytes(health.into_body(), 4096).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["persistence"]["state"], "pending");

        wait_for_persistence(&app, "idle").await;
        assert!(!journal_path.exists());
        let restarted = CatalogStore::new(fixture.root.clone()).load().unwrap();
        assert_eq!(restarted.get(&project.id).unwrap().revision, 1);
    }

    #[test]
    fn metadata_listing_does_not_deserialize_project_snapshots() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        catalog.create("Delivery".to_owned()).unwrap();
        catalog.create("Reliability".to_owned()).unwrap();
        store.save(&mut catalog).unwrap();
        fs::write(
            fixture
                .root
                .join(PROJECTS_DIRECTORY)
                .join("B")
                .join(PROJECT_SNAPSHOT_FILE),
            b"not json",
        )
        .unwrap();

        assert_eq!(store.list_project_metadata().unwrap(), catalog.list());
        assert!(matches!(
            store.load(),
            Err(CatalogPersistenceError::Json { .. })
        ));
    }

    #[test]
    fn startup_repairs_stale_metadata_from_the_authoritative_project_snapshot() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        store.save(&mut catalog).unwrap();
        let metadata_path = fixture
            .root
            .join(PROJECTS_DIRECTORY)
            .join(project.id.as_str())
            .join(PROJECT_METADATA_FILE);
        let stale = fs::read(&metadata_path).unwrap();
        catalog.execute(&project.id, node_request()).unwrap();
        let projects = [project.id.clone()].into_iter().collect();
        store
            .save_projects_unlocked(&mut catalog, &projects)
            .unwrap();
        fs::write(&metadata_path, stale).unwrap();

        let restored = store.load().unwrap();
        assert_eq!(restored.get(&project.id).unwrap().revision, 1);
        let metadata: ProjectMetadataDocument =
            serde_json::from_slice(&fs::read(metadata_path).unwrap()).unwrap();
        assert_eq!(metadata.project.unwrap().revision, 1);
    }

    #[tokio::test]
    async fn compaction_writes_only_the_touched_project_directory() {
        let fixture = Fixture::new();
        let store = CatalogStore::new(fixture.root.clone());
        let mut catalog = ProjectCatalog::new();
        catalog.create("Delivery".to_owned()).unwrap();
        catalog.create("Reliability".to_owned()).unwrap();
        store.save(&mut catalog).unwrap();
        let untouched = fixture
            .root
            .join(PROJECTS_DIRECTORY)
            .join("B")
            .join(PROJECT_SNAPSHOT_FILE);
        let before = fs::read(&untouched).unwrap();
        let app = router_with_persistent_catalog(catalog, store);

        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&node_request()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        wait_for_persistence(&app, "idle").await;
        assert_eq!(fs::read(untouched).unwrap(), before);
    }

    #[tokio::test]
    async fn background_snapshot_failure_is_visible_and_keeps_the_journal() {
        let fixture = Fixture::new();
        let mut catalog = ProjectCatalog::new();
        catalog.create("Delivery".to_owned()).unwrap();
        let store = CatalogStore::new(fixture.root.clone());
        store.save(&mut catalog).unwrap();
        let project_directory = fixture.root.join(PROJECTS_DIRECTORY).join("A");
        let snapshot_path = project_directory.join(PROJECT_SNAPSHOT_FILE);
        let journal_path = project_directory.join("journal.json");
        fs::remove_file(&snapshot_path).unwrap();
        fs::create_dir(&snapshot_path).unwrap();
        let app = router_with_persistent_catalog(catalog, store);

        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/projects/A/commands")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&node_request()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let health = wait_for_persistence(&app, "error").await;
        assert_eq!(health["status"], "degraded");
        assert!(health["persistence"]["error"].as_str().is_some());
        assert!(journal_path.exists());
    }
}
