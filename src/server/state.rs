use std::{collections::BTreeMap, sync::Arc};

use tokio::sync::{RwLock, broadcast};

use crate::{command::ChangeSet, domain::ProjectId, project::ProjectCatalog};

#[derive(Clone)]
pub(super) struct AppState {
    pub(super) catalog: Arc<RwLock<ProjectCatalog>>,
    channels: Arc<RwLock<BTreeMap<ProjectId, broadcast::Sender<ChangeSet>>>>,
    channel_capacity: usize,
}

impl AppState {
    pub(super) fn new(catalog: ProjectCatalog) -> Self {
        Self::with_channel_capacity(catalog, 256)
    }

    fn with_channel_capacity(catalog: ProjectCatalog, channel_capacity: usize) -> Self {
        Self {
            catalog: Arc::new(RwLock::new(catalog)),
            channels: Arc::new(RwLock::new(BTreeMap::new())),
            channel_capacity,
        }
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
        let state = AppState::with_channel_capacity(ProjectCatalog::new(), 1);
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
