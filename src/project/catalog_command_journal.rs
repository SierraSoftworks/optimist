use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    command::{ChangeSet, CommandRequest},
    domain::ProjectId,
};

use super::{
    CatalogPersistenceError, CatalogStore, ProjectCatalog, catalog_persistence::atomic_write,
};

const JOURNAL_FILE: &str = "command-journal.json";
const JOURNAL_SCHEMA_VERSION: u32 = 1;
const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
struct PendingCommand {
    schema_version: u32,
    project: ProjectId,
    request: CommandRequest,
}

impl CatalogStore {
    pub(crate) fn write_pending_command(
        &self,
        project: &ProjectId,
        request: &CommandRequest,
    ) -> Result<(), CatalogPersistenceError> {
        let pending = PendingCommand {
            schema_version: JOURNAL_SCHEMA_VERSION,
            project: project.clone(),
            request: request.clone(),
        };
        let bytes = serde_json::to_vec(&pending).expect("pending commands serialize");
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(CatalogPersistenceError::TooLarge {
                path: self.journal_path(),
            });
        }
        fs::create_dir_all(&self.root).map_err(|source| journal_io(self.root.clone(), source))?;
        atomic_write(&self.root, JOURNAL_FILE, &bytes)
    }

    pub(crate) fn recover_pending_command(
        &self,
        catalog: &mut ProjectCatalog,
    ) -> Result<Option<(ProjectId, ChangeSet)>, CatalogPersistenceError> {
        let Some(pending) = self.read_pending_command()? else {
            return Ok(None);
        };
        let before = catalog.get(&pending.project)?.revision;
        let result = catalog.execute(&pending.project, pending.request)?;
        let change = if result.project_revision > before {
            catalog
                .get_change(&pending.project, result.project_revision)?
                .map(|change| (pending.project, change))
        } else {
            None
        };
        self.save(catalog)?;
        self.clear_pending_command()?;
        Ok(change)
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

    fn read_pending_command(&self) -> Result<Option<PendingCommand>, CatalogPersistenceError> {
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
        let pending: PendingCommand =
            serde_json::from_slice(&bytes).map_err(|source| CatalogPersistenceError::Json {
                path: path.clone(),
                source,
            })?;
        if pending.schema_version != JOURNAL_SCHEMA_VERSION {
            return Err(CatalogPersistenceError::UnsupportedJournalSchema(
                pending.schema_version,
            ));
        }
        Ok(Some(pending))
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
        command::{CommandRequest, CreateNode, GraphCommand},
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
    fn unsupported_journal_schema_stops_startup_without_rewriting_it() {
        let (root, store, _catalog, project) = fixture();
        let path = root.join(JOURNAL_FILE);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": 2,
            "project": project,
            "request": request(),
        }))
        .unwrap();
        fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogPersistenceError::UnsupportedJournalSchema(2))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
