use crate::{
    command::{ChangeSet, CommandBatchRequest, CommandBatchResult, CommandRequest, CommandResult},
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
