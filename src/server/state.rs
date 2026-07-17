use std::{collections::BTreeMap, sync::Arc};

use thiserror::Error;
use tokio::sync::{RwLock, broadcast};

use crate::{
    command::ChangeSet,
    domain::ProjectId,
    project::{
        BackupError, CatalogBackup, CatalogPersistenceError, CatalogRestore, CatalogStore,
        ProjectArchive, ProjectCatalog, ProjectError, ProjectSnapshot,
    },
};

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) catalog: Arc<RwLock<ProjectCatalog>>,
    store: Option<Arc<CatalogStore>>,
    channels: Arc<RwLock<BTreeMap<ProjectId, broadcast::Sender<ChangeSet>>>>,
    channel_capacity: usize,
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
        Self {
            catalog: Arc::new(RwLock::new(catalog)),
            store,
            channels: Arc::new(RwLock::new(BTreeMap::new())),
            channel_capacity,
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
        store.save(&mut candidate)?;
        *catalog = candidate;
        Ok(result)
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
