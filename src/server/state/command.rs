use crate::{
    command::{ChangeSet, CommandRequest, CommandResult},
    domain::ProjectId,
    project::{ProjectCatalog, ProjectError},
};

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

        let mut changes = store
            .recover_pending_command(&mut catalog)?
            .into_iter()
            .collect::<Vec<_>>();
        let before = catalog.get(project)?.revision;
        let mut candidate = catalog.transaction_clone()?;
        let result = candidate.execute(project, request.clone())?;
        let change = committed_change(&mut candidate, project, before, &result)?;
        if let Some(change) = change {
            store.write_pending_command(project, &request)?;
            store.save(&mut candidate)?;
            store.clear_pending_command()?;
            *catalog = candidate;
            changes.push((project.clone(), change));
        }
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
