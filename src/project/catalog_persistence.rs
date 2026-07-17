use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::command::{ChangeSet, CommandResult};

use super::{ProjectArchive, ProjectCatalog, ProjectError};

const SNAPSHOT_SCHEMA_VERSION: u32 = 1;
const SNAPSHOT_FILE: &str = "catalog.json";
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
    /// One embedded canonical project archive or allocator failed validation.
    #[error("catalog snapshot contains invalid project state")]
    Project(#[from] ProjectError),
}

pub(crate) struct CatalogStore {
    root: PathBuf,
}

impl CatalogStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn load(&self) -> Result<ProjectCatalog, CatalogPersistenceError> {
        let path = self.root.join(SNAPSHOT_FILE);
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
        let snapshot: CatalogSnapshot =
            serde_json::from_slice(&bytes).map_err(|source| CatalogPersistenceError::Json {
                path: path.clone(),
                source,
            })?;
        ProjectCatalog::from_persisted_snapshot(snapshot)
    }

    pub(crate) fn save(&self, catalog: &mut ProjectCatalog) -> Result<(), CatalogPersistenceError> {
        fs::create_dir_all(&self.root).map_err(|source| io_error(self.root.clone(), source))?;
        let snapshot = catalog.persisted_snapshot()?;
        let bytes = serde_json::to_vec(&snapshot).expect("catalog snapshots serialize");
        if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
            return Err(CatalogPersistenceError::TooLarge {
                path: self.root.join(SNAPSHOT_FILE),
            });
        }
        atomic_write(&self.root, SNAPSHOT_FILE, &bytes)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CatalogSnapshot {
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
    #[serde(default)]
    change_history_start: Option<u64>,
    #[serde(default)]
    changes: Vec<ChangeSet>,
}

impl ProjectCatalog {
    pub(crate) fn transaction_clone(&mut self) -> Result<Self, CatalogPersistenceError> {
        let snapshot = self.persisted_snapshot()?;
        let mut candidate = Self::from_persisted_snapshot(snapshot)?;
        for (id, entry) in &self.projects {
            let cloned = candidate
                .projects
                .get_mut(id)
                .expect("snapshot clone retains every project");
            cloned.results = entry.results.clone();
        }
        Ok(candidate)
    }

    fn persisted_snapshot(&mut self) -> Result<CatalogSnapshot, ProjectError> {
        let ids = self.projects.keys().cloned().collect::<Vec<_>>();
        let mut projects = Vec::with_capacity(ids.len());
        for id in ids {
            let archive = self.export_archive(&id)?;
            let entry = self
                .projects
                .get(&id)
                .expect("project ID came from catalog");
            projects.push(PersistedProject {
                archive,
                graph_revision: entry.graph_revision,
                next_entity_id: entry.repository.next_entity_id_counter(),
                next_scenario_id: entry.next_scenario_id,
                change_history_start: Some(entry.change_history_start),
                changes: entry.changes.values().cloned().collect(),
            });
        }
        Ok(CatalogSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            next_project_id: self.next_project_id,
            projects,
        })
    }

    fn from_persisted_snapshot(snapshot: CatalogSnapshot) -> Result<Self, CatalogPersistenceError> {
        if snapshot.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(CatalogPersistenceError::UnsupportedSchema(
                snapshot.schema_version,
            ));
        }
        let mut catalog = Self::new();
        for persisted in snapshot.projects {
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
            restore_changes(
                entry,
                persisted.change_history_start.unwrap_or({
                    if persisted.changes.is_empty() {
                        persisted.archive.project.revision
                    } else {
                        0
                    }
                }),
                persisted.changes,
            )?;
            entry
                .repository
                .restore_next_entity_id_counter(persisted.next_entity_id)
                .map_err(ProjectError::from)?;
        }
        catalog.next_project_id = snapshot.next_project_id;
        Ok(catalog)
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

fn atomic_write(root: &Path, name: &str, bytes: &[u8]) -> Result<(), CatalogPersistenceError> {
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
            CommandRequest, CreateNode, CreateScenario, DeleteNode, DeleteScenario, GraphCommand,
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
        let path = fixture.root.join(SNAPSHOT_FILE);
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
        let snapshot_path = fixture.root.join(SNAPSHOT_FILE);
        fs::create_dir(&snapshot_path).unwrap();
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
    }
}
