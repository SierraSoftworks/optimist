use std::{
    collections::BTreeMap,
    sync::{
        Arc, RwLock as StdRwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
use thiserror::Error;
use tokio::sync::{RwLock, broadcast, mpsc};

use crate::{
    command::ChangeSet,
    domain::ProjectId,
    project::{
        BackupError, CatalogBackup, CatalogPersistenceError, CatalogRestore, CatalogStore,
        ProjectArchive, ProjectCatalog, ProjectError, ProjectSnapshot,
    },
};

use super::bounded_worker::BoundedWorker;

mod command;

const ANALYSIS_WORKERS: usize = 4;
const ANALYSIS_QUEUE_TIMEOUT: Duration = Duration::from_millis(100);
const ANALYSIS_EXECUTION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) catalog: Arc<RwLock<ProjectCatalog>>,
    store: Option<Arc<CatalogStore>>,
    channels: Arc<RwLock<BTreeMap<ProjectId, broadcast::Sender<ChangeSet>>>>,
    channel_capacity: usize,
    persistence_tx: Option<mpsc::UnboundedSender<()>>,
    persistence_status: Arc<StdRwLock<PersistenceStatus>>,
    generation: Arc<AtomicU64>,
    pub(super) analysis_worker: BoundedWorker,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct PersistenceStatus {
    /// Current snapshot compaction state.
    pub(super) state: &'static str,
    /// Most recent background compaction failure, when persistence is degraded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl AppState {
    pub(super) fn new(catalog: ProjectCatalog) -> Self {
        Self::with_channel_capacity(catalog, None, 256)
    }

    pub(super) fn persistent(catalog: ProjectCatalog, store: CatalogStore) -> Self {
        Self::with_channel_capacity(catalog, Some(Arc::new(store)), 256)
    }

    fn with_channel_capacity(
        catalog: ProjectCatalog,
        store: Option<Arc<CatalogStore>>,
        channel_capacity: usize,
    ) -> Self {
        let catalog = Arc::new(RwLock::new(catalog));
        let channels = Arc::new(RwLock::new(BTreeMap::new()));
        let persistence_status = Arc::new(StdRwLock::new(PersistenceStatus {
            state: "idle",
            error: None,
        }));
        let generation = Arc::new(AtomicU64::new(0));
        let persistence_tx = store.as_ref().map(|store| {
            let (tx, rx) = mpsc::unbounded_channel();
            spawn_persistence_worker(
                Arc::clone(&catalog),
                Arc::clone(store),
                Arc::clone(&persistence_status),
                Arc::clone(&generation),
                rx,
            );
            tx
        });
        Self {
            catalog,
            store,
            channels,
            channel_capacity,
            persistence_tx,
            persistence_status,
            generation,
            analysis_worker: BoundedWorker::new(
                ANALYSIS_WORKERS,
                ANALYSIS_QUEUE_TIMEOUT,
                ANALYSIS_EXECUTION_TIMEOUT,
            ),
        }
    }

    pub(super) async fn mutate<T>(
        &self,
        operation: impl FnOnce(&mut ProjectCatalog) -> Result<T, ProjectError>,
    ) -> Result<T, CatalogMutationError> {
        let mut catalog = self.catalog.write().await;
        let Some(store) = &self.store else {
            return operation(&mut catalog).map_err(CatalogMutationError::from);
        };
        let mut candidate = catalog.transaction_clone()?;
        let result = operation(&mut candidate)?;
        self.generation.fetch_add(1, Ordering::AcqRel);
        let mutation_counts = store.pending_mutation_counts()?;
        store.save(&mut candidate)?;
        store.compact_pending_mutations(&mutation_counts)?;
        *catalog = candidate;
        Ok(result)
    }

    pub(super) fn schedule_persistence(&self) {
        let Some(tx) = &self.persistence_tx else {
            return;
        };
        self.persistence_status
            .write()
            .expect("persistence status lock poisoned")
            .state = "pending";
        if tx.send(()).is_err() {
            *self
                .persistence_status
                .write()
                .expect("persistence status lock poisoned") = PersistenceStatus {
                state: "error",
                error: Some("background persistence worker stopped".to_owned()),
            };
        }
    }

    pub(super) fn persistence_status(&self) -> PersistenceStatus {
        self.persistence_status
            .read()
            .expect("persistence status lock poisoned")
            .clone()
    }

    pub(super) async fn subscribe(&self, project: &ProjectId) -> broadcast::Receiver<ChangeSet> {
        let mut channels = self.channels.write().await;
        channels
            .entry(project.clone())
            .or_insert_with(|| broadcast::channel(self.channel_capacity).0)
            .subscribe()
    }

    pub(super) async fn publish(&self, project: &ProjectId, change: ChangeSet) {
        let channels = self.channels.read().await;
        if let Some(channel) = channels.get(project) {
            let _ = channel.send(change);
        }
    }

    pub(super) async fn create_backup(&self) -> Result<CatalogBackup, BackupError> {
        let store = self.store.as_ref().ok_or(BackupError::Unavailable)?;
        let mut catalog = self.catalog.write().await;
        store.create_backup(&mut catalog)
    }

    pub(super) fn list_backups(&self) -> Result<Vec<CatalogBackup>, BackupError> {
        self.store
            .as_ref()
            .ok_or(BackupError::Unavailable)?
            .list_backups()
    }

    pub(super) async fn restore_backup(
        &self,
        id: uuid::Uuid,
        confirmed: bool,
    ) -> Result<CatalogRestore, BackupError> {
        if !confirmed {
            return Err(BackupError::ConfirmationRequired);
        }
        let store = self.store.as_ref().ok_or(BackupError::Unavailable)?;
        let mut catalog = self.catalog.write().await;
        let (restored, mut replacement) = store.load_backup(id)?;
        let safety_backup = store.create_backup(&mut catalog)?;
        store.save(&mut replacement)?;
        *catalog = replacement;
        self.channels.write().await.clear();
        Ok(CatalogRestore {
            restored,
            safety_backup,
            projects: catalog.list(),
        })
    }

    pub(super) async fn create_project_snapshot(
        &self,
        project: &ProjectId,
    ) -> Result<ProjectSnapshot, BackupError> {
        let store = self.store.as_ref().ok_or(BackupError::Unavailable)?;
        let mut catalog = self.catalog.write().await;
        store.create_project_snapshot(&mut catalog, project)
    }

    pub(super) fn list_project_snapshots(
        &self,
        project: &ProjectId,
    ) -> Result<Vec<ProjectSnapshot>, BackupError> {
        self.store
            .as_ref()
            .ok_or(BackupError::Unavailable)?
            .list_project_snapshots(project)
    }

    pub(super) fn get_project_snapshot(
        &self,
        project: &ProjectId,
        revision: u64,
    ) -> Result<ProjectArchive, BackupError> {
        self.store
            .as_ref()
            .ok_or(BackupError::Unavailable)?
            .get_project_snapshot(project, revision)
    }
}

fn spawn_persistence_worker(
    catalog: Arc<RwLock<ProjectCatalog>>,
    store: Arc<CatalogStore>,
    status: Arc<StdRwLock<PersistenceStatus>>,
    generation: Arc<AtomicU64>,
    mut receiver: mpsc::UnboundedReceiver<()>,
) {
    tokio::spawn(async move {
        while receiver.recv().await.is_some() {
            loop {
                match tokio::time::timeout(Duration::from_millis(250), receiver.recv()).await {
                    Ok(Some(())) => continue,
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            let snapshot = {
                let catalog = catalog.write().await;
                let expected_generation = generation.load(Ordering::Acquire);
                match store.pending_mutation_counts() {
                    Ok(mutation_counts) => {
                        let projects = mutation_counts.keys().cloned().collect();
                        catalog
                            .transaction_clone_projects(&projects)
                            .map(|candidate| {
                                (candidate, projects, expected_generation, mutation_counts)
                            })
                    }
                    Err(error) => Err(error),
                }
            };
            let result = match snapshot {
                Ok((mut candidate, projects, expected_generation, mutation_counts)) => {
                    let store = Arc::clone(&store);
                    let generation = Arc::clone(&generation);
                    tokio::task::spawn_blocking(move || {
                        if store.save_if_current(
                            &mut candidate,
                            &projects,
                            &generation,
                            expected_generation,
                        )? {
                            store.compact_pending_mutations(&mutation_counts)?;
                        }
                        Ok::<(), CatalogPersistenceError>(())
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()))
                }
                Err(error) => Err(error.to_string()),
            };
            *status.write().expect("persistence status lock poisoned") = match result {
                Ok(()) => match store.pending_mutation_counts() {
                    Ok(counts) if counts.is_empty() => PersistenceStatus {
                        state: "idle",
                        error: None,
                    },
                    Ok(_) => PersistenceStatus {
                        state: "pending",
                        error: None,
                    },
                    Err(error) => PersistenceStatus {
                        state: "error",
                        error: Some(error.to_string()),
                    },
                },
                Err(error) => PersistenceStatus {
                    state: "error",
                    error: Some(error),
                },
            };
        }
    });
}

#[derive(Debug, Error)]
pub(super) enum CatalogMutationError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Persistence(#[from] CatalogPersistenceError),
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{ChangeSet, CommandOutcome, GraphCommand},
        domain::ProjectId,
        project::ProjectCatalog,
    };

    use super::AppState;

    fn change(revision: u64) -> ChangeSet {
        ChangeSet {
            request_id: uuid::Uuid::new_v4(),
            batch_id: None,
            compensates: None,
            base_revision: revision - 1,
            project_revision: revision,
            graph_revision: revision,
            command: GraphCommand::DeleteNode(crate::command::DeleteNode {
                id: crate::domain::EntityId::new(0),
            }),
            outcome: CommandOutcome::NodeDeleted(
                crate::domain::Node::new(
                    crate::domain::EntityId::new(0),
                    "node",
                    "Node",
                    crate::domain::NodePayload::Factor(crate::domain::Factor {
                        current: None,
                        desired: None,
                        controllable: false,
                        evidence: vec![],
                    }),
                )
                .unwrap(),
            ),
        }
    }

    #[tokio::test]
    async fn bounded_project_channels_report_lag() {
        let state = AppState::with_channel_capacity(ProjectCatalog::new(), None, 1);
        let project = ProjectId::new("A").unwrap();
        let mut receiver = state.subscribe(&project).await;
        state.publish(&project, change(1)).await;
        state.publish(&project, change(2)).await;
        assert!(matches!(
            receiver.recv().await,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
        ));
    }
}
