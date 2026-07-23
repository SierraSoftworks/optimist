use crate::{
    command::{
        ChangeSet, CommandBatchRequest, CommandBatchResult, CommandRequest, CommandResult,
        GraphCommand,
    },
    domain::ProjectId,
    project::{ProjectCatalog, ProjectError},
};
use std::sync::atomic::Ordering;

use super::{AppState, CatalogMutationError};

impl AppState {
    pub(in crate::server) async fn execute_command(
        &self,
        project: &ProjectId,
        request: CommandRequest,
    ) -> Result<(CommandResult, Vec<(ProjectId, ChangeSet)>), CatalogMutationError> {
        let mut catalog = self.catalog.write().await;
        let Some(store) = &self.store else {
            let before = catalog.get(project)?.revision;
            let result = catalog.execute(project, request)?;
            let changes = committed_change(&mut catalog, project, before, &result)?
                .into_iter()
                .map(|change| (project.clone(), change))
                .collect();
            return Ok((result, changes));
        };

        if matches!(
            &request.command,
            GraphCommand::SetEstimate(_)
                | GraphCommand::SetFermiEstimate(_)
                | GraphCommand::SetSquiggleEstimate(_)
        ) {
            return self.execute_estimate_command(&mut catalog, store, project, request);
        }

        let before = catalog.get(project)?.revision;
        let mut candidate = catalog.project_transaction_clone(project)?;
        let result = candidate.execute(project, request.clone())?;
        let change = committed_change(&mut candidate, project, before, &result)?;
        if let Some(change) = change {
            store.write_pending_command(project, &request)?;
            catalog.publish_project_transaction(project, candidate)?;
            self.generation.fetch_add(1, Ordering::AcqRel);
            self.schedule_persistence();
            return Ok((result, vec![(project.clone(), change)]));
        }
        Ok((result, vec![]))
    }

    fn execute_estimate_command(
        &self,
        catalog: &mut ProjectCatalog,
        store: &crate::project::CatalogStore,
        project: &ProjectId,
        request: CommandRequest,
    ) -> Result<(CommandResult, Vec<(ProjectId, ChangeSet)>), CatalogMutationError> {
        if let Some(result) = catalog.command_preflight(project, &request)? {
            return Ok((result, vec![]));
        }
        let before = catalog.get(project)?.revision;
        let rollback = catalog.estimate_transaction_snapshot(project, &request.command)?;
        let result = catalog.execute(project, request.clone())?;
        let change = committed_change(catalog, project, before, &result)?;
        let Some(change) = change else {
            return Ok((result, vec![]));
        };
        if let Err(error) = store.write_pending_command(project, &request) {
            catalog.rollback_estimate_transaction(project, rollback, &result)?;
            return Err(error.into());
        }
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.schedule_persistence();
        Ok((result, vec![(project.clone(), change)]))
    }

    pub(in crate::server) async fn execute_batch(
        &self,
        project: &ProjectId,
        request: CommandBatchRequest,
        compensates: Option<uuid::Uuid>,
    ) -> Result<(CommandBatchResult, Vec<ChangeSet>), CatalogMutationError> {
        let mut catalog = self.catalog.write().await;
        let before = catalog.get(project)?.revision;
        let mut candidate = catalog.project_transaction_clone(project)?;
        let result = candidate.execute_batch(project, request.clone(), compensates)?;
        let changes = result
            .results
            .iter()
            .filter(|result| result.project_revision > before)
            .filter_map(|result| {
                candidate
                    .get_change(project, result.project_revision)
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if changes.is_empty() {
            return Ok((result, changes));
        }
        if let Some(store) = &self.store {
            store.write_pending_batch(project, &request, compensates)?;
        }
        catalog.publish_project_transaction(project, candidate)?;
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.schedule_persistence();
        Ok((result, changes))
    }
}

fn committed_change(
    catalog: &mut ProjectCatalog,
    project: &ProjectId,
    before: u64,
    result: &CommandResult,
) -> Result<Option<ChangeSet>, ProjectError> {
    if result.project_revision > before {
        catalog.get_change(project, result.project_revision)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        command::{CommandRequest, CreateNode, GraphCommand, SetEstimate},
        domain::{
            Distribution, EntityId, EstimateAddress, EstimateId, EstimateOwner, EstimateSlot,
            EstimateUncertainty, Factor, NodePayload,
        },
        project::{CatalogStore, ProjectCatalog},
    };

    use super::{AppState, CatalogMutationError};

    #[tokio::test]
    async fn failed_estimate_journal_restores_aggregate_and_revisions() {
        let root = std::env::temp_dir().join(format!(
            "optimist-estimate-rollback-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("projects"), "blocks project directories").unwrap();
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Rollback".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "flow".to_owned(),
                        title: "Flow".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            current: None,
                            desired: None,
                            controllable: false,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let address = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::beta(2.0, 2.0).unwrap(),
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let state = AppState::persistent(catalog, CatalogStore::new(root.clone()));
        let request = CommandRequest::new(
            2,
            GraphCommand::SetEstimate(SetEstimate {
                address: address.clone(),
                slot: EstimateSlot::Current,
                distribution: Distribution::beta(8.0, 2.0).unwrap(),
                provenance: vec![],
                uncertainty: EstimateUncertainty::default(),
            }),
        );

        assert!(matches!(
            state.execute_command(&project.id, request).await,
            Err(CatalogMutationError::Persistence(_))
        ));
        let mut restored = state.catalog.write().await;
        assert_eq!(restored.get(&project.id).unwrap().revision, 2);
        assert_eq!(
            restored
                .get_estimate(&project.id, &address)
                .unwrap()
                .distribution,
            Distribution::beta(2.0, 2.0).unwrap()
        );
        drop(restored);
        fs::remove_dir_all(root).unwrap();
    }
}
