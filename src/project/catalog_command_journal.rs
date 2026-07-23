use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    command::{ChangeSet, CommandBatchRequest, CommandRequest},
    domain::ProjectId,
};

use super::{
    CatalogPersistenceError, CatalogStore, ProjectCatalog, ProjectError,
    catalog_persistence::atomic_write,
    command_journal_document::{PendingMutation, decode, encode},
};

const PROJECTS_DIRECTORY: &str = "projects";
const JOURNAL_FILE: &str = "journal.json";
const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;

impl CatalogStore {
    pub(crate) fn write_pending_command(
        &self,
        project: &ProjectId,
        request: &CommandRequest,
    ) -> Result<(), CatalogPersistenceError> {
        self.append_pending_mutation(
            project,
            PendingMutation::Command {
                project: project.clone(),
                request: Box::new(request.clone()),
            },
        )
    }

    pub(crate) fn write_pending_batch(
        &self,
        project: &ProjectId,
        request: &CommandBatchRequest,
        compensates: Option<uuid::Uuid>,
    ) -> Result<(), CatalogPersistenceError> {
        self.append_pending_mutation(
            project,
            PendingMutation::Batch {
                project: project.clone(),
                request: request.clone(),
                compensates,
            },
        )
    }

    fn append_pending_mutation(
        &self,
        project: &ProjectId,
        mutation: PendingMutation,
    ) -> Result<(), CatalogPersistenceError> {
        let _guard = self.journal_lock.lock().expect("journal lock poisoned");
        let path = self.project_journal_path(project);
        let mut mutations = self.read_journal_unlocked(&path)?;
        mutations.push(mutation);
        let bytes = encode(mutations);
        if bytes.len() as u64 > MAX_JOURNAL_BYTES {
            return Err(CatalogPersistenceError::TooLarge { path });
        }
        let directory = path.parent().expect("project journals have a parent");
        fs::create_dir_all(directory)
            .map_err(|source| journal_io(directory.to_path_buf(), source))?;
        atomic_write(directory, JOURNAL_FILE, &bytes)
    }

    pub(crate) fn recover_pending_mutations(
        &self,
        catalog: &mut ProjectCatalog,
    ) -> Result<Vec<(ProjectId, ChangeSet)>, CatalogPersistenceError> {
        let pending = self.read_pending_mutations()?;
        if pending.is_empty() {
            return Ok(vec![]);
        }
        let mut changes = Vec::new();
        for mutation in pending {
            match mutation {
                PendingMutation::Command { project, request } => {
                    let before = catalog.get(&project)?.revision;
                    let result = catalog.execute(&project, *request)?;
                    if result.project_revision > before
                        && let Some(change) =
                            catalog.get_change(&project, result.project_revision)?
                    {
                        changes.push((project, change));
                    }
                }
                PendingMutation::Batch {
                    project,
                    request,
                    compensates,
                } => {
                    let before = catalog.get(&project)?.revision;
                    let result = catalog.execute_batch(&project, request, compensates)?;
                    changes.extend(
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
                            .map(|change| (project.clone(), change)),
                    );
                }
            }
        }
        self.save(catalog)?;
        self.clear_pending_mutations()?;
        Ok(changes)
    }

    pub(crate) fn pending_mutation_counts(
        &self,
    ) -> Result<BTreeMap<ProjectId, usize>, CatalogPersistenceError> {
        let _guard = self.journal_lock.lock().expect("journal lock poisoned");
        self.read_project_journals_unlocked().map(|journals| {
            journals
                .into_iter()
                .map(|(project, mutations)| (project, mutations.len()))
                .filter(|(_, count)| *count > 0)
                .collect()
        })
    }

    pub(crate) fn compact_pending_mutations(
        &self,
        counts: &BTreeMap<ProjectId, usize>,
    ) -> Result<(), CatalogPersistenceError> {
        let _guard = self.journal_lock.lock().expect("journal lock poisoned");
        for (project, count) in counts {
            let path = self.project_journal_path(project);
            let mut mutations = self.read_journal_unlocked(&path)?;
            mutations.drain(..(*count).min(mutations.len()));
            if mutations.is_empty() {
                self.remove_journal_unlocked(&path)?;
            } else {
                atomic_write(
                    path.parent().expect("project journals have a parent"),
                    JOURNAL_FILE,
                    &encode(mutations),
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn clear_pending_mutations(&self) -> Result<(), CatalogPersistenceError> {
        let _guard = self.journal_lock.lock().expect("journal lock poisoned");
        for project in self.read_project_journals_unlocked()?.keys() {
            self.remove_journal_unlocked(&self.project_journal_path(project))?;
        }
        Ok(())
    }

    fn remove_journal_unlocked(&self, path: &Path) -> Result<(), CatalogPersistenceError> {
        let directory = path.parent().unwrap_or(&self.root);
        match fs::remove_file(path) {
            Ok(()) => fs::File::open(directory)
                .and_then(|directory| directory.sync_all())
                .map_err(|source| journal_io(directory.to_path_buf(), source)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(journal_io(path.to_path_buf(), source)),
        }
    }

    fn read_pending_mutations(&self) -> Result<Vec<PendingMutation>, CatalogPersistenceError> {
        let _guard = self.journal_lock.lock().expect("journal lock poisoned");
        self.read_pending_mutations_unlocked()
    }

    fn read_pending_mutations_unlocked(
        &self,
    ) -> Result<Vec<PendingMutation>, CatalogPersistenceError> {
        let mut mutations = Vec::new();
        for (_, project_mutations) in self.read_project_journals_unlocked()? {
            mutations.extend(project_mutations);
        }
        Ok(mutations)
    }

    fn read_project_journals_unlocked(
        &self,
    ) -> Result<BTreeMap<ProjectId, Vec<PendingMutation>>, CatalogPersistenceError> {
        let directory = self.root.join(PROJECTS_DIRECTORY);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(BTreeMap::new());
            }
            Err(source) => return Err(journal_io(directory, source)),
        };
        let mut journals = BTreeMap::new();
        for entry in entries {
            let entry = entry.map_err(|source| journal_io(directory.clone(), source))?;
            if !entry
                .file_type()
                .map_err(|source| journal_io(entry.path(), source))?
                .is_dir()
            {
                continue;
            }
            let project = ProjectId::new(entry.file_name().to_string_lossy()).map_err(|error| {
                CatalogPersistenceError::Project(ProjectError::InvalidArchivePath(format!(
                    "invalid project journal directory: {error}"
                )))
            })?;
            let mutations = self.read_journal_unlocked(&entry.path().join(JOURNAL_FILE))?;
            if !mutations.is_empty() {
                journals.insert(project, mutations);
            }
        }
        Ok(journals)
    }

    fn read_journal_unlocked(
        &self,
        path: &Path,
    ) -> Result<Vec<PendingMutation>, CatalogPersistenceError> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(source) => return Err(journal_io(path.to_path_buf(), source)),
        };
        if metadata.len() > MAX_JOURNAL_BYTES {
            return Err(CatalogPersistenceError::TooLarge {
                path: path.to_path_buf(),
            });
        }
        let bytes = fs::read(path).map_err(|source| journal_io(path.to_path_buf(), source))?;
        decode(&bytes, path)
    }

    fn project_journal_path(&self, project: &ProjectId) -> PathBuf {
        self.root
            .join(PROJECTS_DIRECTORY)
            .join(project.as_str())
            .join(JOURNAL_FILE)
    }
}

fn journal_io(path: PathBuf, source: std::io::Error) -> CatalogPersistenceError {
    CatalogPersistenceError::Io { path, source }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use crate::{
        command::{CommandBatchRequest, CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload, ProjectId},
        project::{CatalogPersistenceError, CatalogStore, ProjectCatalog},
    };

    use super::{JOURNAL_FILE, PROJECTS_DIRECTORY};

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
                        controllable: false,
                        evidence: vec![],
                    }),
                }),
            ],
        }
    }

    fn project_journal(root: &std::path::Path, project: &ProjectId) -> PathBuf {
        root.join(PROJECTS_DIRECTORY)
            .join(project.as_str())
            .join(JOURNAL_FILE)
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
        assert!(!project_journal(&root, &project).exists());

        let mut restarted = store.load().unwrap();
        assert_eq!(restarted.get(&project).unwrap().revision, 1);
        assert_eq!(restarted.list_nodes(&project).unwrap().len(), 1);
        let retry = restarted.execute(&project, request()).unwrap();
        assert_eq!(retry.project_revision, 1);
        assert_eq!(restarted.list_nodes(&project).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_applies_multiple_ordered_journal_mutations_once() {
        let (root, store, _catalog, project) = fixture();
        let first = request();
        let mut second = request();
        second.request_id = Uuid::new_v4();
        second.expected_revision = 1;
        let GraphCommand::CreateNode(node) = &mut second.command else {
            unreachable!()
        };
        node.name = "quality".to_owned();
        node.title = "Quality".to_owned();
        store.write_pending_command(&project, &first).unwrap();
        store.write_pending_command(&project, &second).unwrap();

        let mut restored = store.load().unwrap();
        assert_eq!(restored.get(&project).unwrap().revision, 2);
        assert_eq!(restored.list_nodes(&project).unwrap().len(), 2);
        assert_eq!(
            restored.replay_changes(&project, 0).unwrap().changes.len(),
            2
        );
        assert!(!project_journal(&root, &project).exists());
        assert_eq!(
            restored.execute(&project, first).unwrap().project_revision,
            1
        );
        assert_eq!(
            restored.execute(&project, second).unwrap().project_revision,
            2
        );
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
        assert!(!project_journal(&root, &project).exists());
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
            assert!(!project_journal(&root, &project).exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn unsupported_journal_schema_stops_startup_without_rewriting_it() {
        let (root, store, _catalog, project) = fixture();
        let path = project_journal(&root, &project);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": 4,
            "mutations": [],
        }))
        .unwrap();
        fs::write(&path, &bytes).unwrap();

        assert!(matches!(
            store.load(),
            Err(CatalogPersistenceError::UnsupportedJournalSchema(4))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
}
