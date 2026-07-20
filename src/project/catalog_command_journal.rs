use std::{fs, path::PathBuf};

use crate::{
    command::{ChangeSet, CommandBatchRequest, CommandRequest},
    domain::ProjectId,
};

use super::{
    CatalogPersistenceError, CatalogStore, ProjectCatalog,
    catalog_persistence::atomic_write,
    command_journal_document::{PendingMutation, decode, encode},
};

const JOURNAL_FILE: &str = "command-journal.json";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;

impl CatalogStore {
    pub(crate) fn write_pending_command(
        &self,
        project: &ProjectId,
        request: &CommandRequest,
    ) -> Result<(), CatalogPersistenceError> {
        self.write_pending_mutation(PendingMutation::Command {
            project: project.clone(),
            request: Box::new(request.clone()),
        })
    }

    pub(crate) fn write_pending_batch(
        &self,
        project: &ProjectId,
        request: &CommandBatchRequest,
        compensates: Option<uuid::Uuid>,
    ) -> Result<(), CatalogPersistenceError> {
        self.write_pending_mutation(PendingMutation::Batch {
            project: project.clone(),
            request: request.clone(),
            compensates,
        })
    }

    fn write_pending_mutation(
        &self,
        mutation: PendingMutation,
    ) -> Result<(), CatalogPersistenceError> {
        let bytes = encode(mutation);
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(CatalogPersistenceError::TooLarge {
                path: self.journal_path(),
            });
        }
        fs::create_dir_all(&self.root).map_err(|source| journal_io(self.root.clone(), source))?;
        atomic_write(&self.root, JOURNAL_FILE, &bytes)
    }

    pub(crate) fn recover_pending_mutation(
        &self,
        catalog: &mut ProjectCatalog,
    ) -> Result<Vec<(ProjectId, ChangeSet)>, CatalogPersistenceError> {
        let Some(pending) = self.read_pending_mutation()? else {
            return Ok(vec![]);
        };
        let changes = match pending {
            PendingMutation::Command { project, request } => {
                let before = catalog.get(&project)?.revision;
                let result = catalog.execute(&project, *request)?;
                if result.project_revision > before {
                    catalog
                        .get_change(&project, result.project_revision)?
                        .map(|change| vec![(project, change)])
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            }
            PendingMutation::Batch {
                project,
                request,
                compensates,
            } => {
                let before = catalog.get(&project)?.revision;
                let result = catalog.execute_batch(&project, request, compensates)?;
                result
                    .results
                    .into_iter()
                    .filter(|result| result.project_revision > before)
                    .filter_map(|result| {
                        catalog
                            .get_change(&project, result.project_revision)
                            .transpose()
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .map(|change| (project.clone(), change))
                    .collect()
            }
        };
        self.save(catalog)?;
        self.clear_pending_command()?;
        Ok(changes)
    }

    pub(crate) fn clear_pending_command(&self) -> Result<(), CatalogPersistenceError> {
        let path = self.journal_path();
        match fs::remove_file(&path) {
            Ok(()) => fs::File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .map_err(|source| journal_io(self.root.clone(), source)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(journal_io(path, source)),
        }
    }

    fn read_pending_mutation(&self) -> Result<Option<PendingMutation>, CatalogPersistenceError> {
        let path = self.journal_path();
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(journal_io(path, source)),
        };
        if metadata.len() > MAX_JOURNAL_BYTES {
            return Err(CatalogPersistenceError::TooLarge { path });
        }
        let bytes = fs::read(&path).map_err(|source| journal_io(path.clone(), source))?;
        decode(&bytes, &path).map(Some)
    }

    fn journal_path(&self) -> PathBuf {
        self.root.join(JOURNAL_FILE)
    }
}

fn journal_io(path: PathBuf, source: std::io::Error) -> CatalogPersistenceError {
    CatalogPersistenceError::Io { path, source }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use crate::{
        command::{CommandBatchRequest, CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload, ProjectId},
        project::{CatalogPersistenceError, CatalogStore, ProjectCatalog},
    };

    use super::JOURNAL_FILE;

    fn fixture() -> (std::path::PathBuf, CatalogStore, ProjectCatalog, ProjectId) {
        let root =
            std::env::temp_dir().join(format!("optimist-command-journal-{}", Uuid::new_v4()));
        let store = CatalogStore::new(root.clone());
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        store.save(&mut catalog).unwrap();
        (root, store, catalog, project.id)
    }

    fn request() -> CommandRequest {
        CommandRequest {
            request_id: Uuid::nil(),
            expected_revision: 0,
            command: GraphCommand::CreateNode(CreateNode {
                name: "flow".to_owned(),
                title: "Flow".to_owned(),
                payload: NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        }
    }

    fn batch() -> CommandBatchRequest {
        CommandBatchRequest {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            commands: vec![
                request().command,
                GraphCommand::CreateNode(CreateNode {
                    name: "quality".to_owned(),
                    title: "Quality".to_owned(),
                    payload: NodePayload::Factor(Factor {
                        current: None,
                        desired: None,
                        controllable: false,
                        evidence: vec![],
                    }),
                }),
            ],
        }
    }

    #[test]
    fn startup_applies_a_journal_written_before_the_catalog_snapshot() {
        let (root, store, _catalog, project) = fixture();
        store.write_pending_command(&project, &request()).unwrap();

        let mut restored = store.load().unwrap();
        assert_eq!(restored.get(&project).unwrap().revision, 1);
        assert_eq!(restored.list_nodes(&project).unwrap().len(), 1);
        assert_eq!(
            restored.replay_changes(&project, 0).unwrap().changes.len(),
            1
        );
        assert!(!root.join(JOURNAL_FILE).exists());

        let mut restarted = store.load().unwrap();
        assert_eq!(restarted.get(&project).unwrap().revision, 1);
        assert_eq!(restarted.list_nodes(&project).unwrap().len(), 1);
        let retry = restarted.execute(&project, request()).unwrap();
        assert_eq!(retry.project_revision, 1);
        assert_eq!(restarted.list_nodes(&project).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_clears_a_journal_left_after_the_catalog_snapshot() {
        let (root, store, mut catalog, project) = fixture();
        store.write_pending_command(&project, &request()).unwrap();
        catalog.execute(&project, request()).unwrap();
        store.save(&mut catalog).unwrap();

        let mut restored = store.load().unwrap();
        assert_eq!(restored.get(&project).unwrap().revision, 1);
        assert_eq!(restored.list_nodes(&project).unwrap().len(), 1);
        assert_eq!(
            restored.replay_changes(&project, 0).unwrap().changes.len(),
            1
        );
        assert!(!root.join(JOURNAL_FILE).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_recovers_each_batch_command_once_across_both_crash_windows() {
        for snapshot_published in [false, true] {
            let (root, store, mut catalog, project) = fixture();
            let batch = batch();
            store.write_pending_batch(&project, &batch, None).unwrap();
            if snapshot_published {
                catalog
                    .execute_batch(&project, batch.clone(), None)
                    .unwrap();
                store.save(&mut catalog).unwrap();
            }

            let mut restored = store.load().unwrap();
            assert_eq!(restored.get(&project).unwrap().revision, 2);
            assert_eq!(restored.list_nodes(&project).unwrap().len(), 2);
            let changes = restored.replay_changes(&project, 0).unwrap().changes;
            assert_eq!(changes.len(), 2);
            assert!(
                changes
                    .iter()
                    .all(|change| change.batch_id == Some(batch.request_id))
            );
            assert!(!root.join(JOURNAL_FILE).exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn startup_recovers_legacy_v1_command_journals() {
        let (root, store, _catalog, project) = fixture();
        let path = root.join(JOURNAL_FILE);
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "project": project,
                "request": request(),
            }))
            .unwrap(),
        )
        .unwrap();

        let mut restored = store.load().unwrap();
        assert_eq!(restored.get(&project).unwrap().revision, 1);
        assert_eq!(restored.list_nodes(&project).unwrap().len(), 1);
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_journal_schema_stops_startup_without_rewriting_it() {
        let (root, store, _catalog, project) = fixture();
        let path = root.join(JOURNAL_FILE);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": 3,
            "project": project,
            "request": request(),
        }))
        .unwrap();
        fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogPersistenceError::UnsupportedJournalSchema(3))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
