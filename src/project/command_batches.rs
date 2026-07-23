use crate::{
    command::{
        CommandBatchRequest, CommandBatchResult, CommandRequest, MAX_COMMAND_BATCH_SIZE,
        child_request_id,
    },
    domain::ProjectId,
};

use super::{
    CommandBatchError, ProjectCatalog, ProjectError,
    command_batch_history::{existing_batch, validate_compensation},
};

impl ProjectCatalog {
    pub(crate) fn execute_batch(
        &mut self,
        project_id: &ProjectId,
        request: CommandBatchRequest,
        compensates: Option<uuid::Uuid>,
    ) -> Result<CommandBatchResult, ProjectError> {
        validate_batch(&request)?;
        if let Some(result) = existing_batch(self, project_id, &request, compensates)? {
            return Ok(result);
        }
        validate_compensation(self, project_id, compensates)?;
        validate_child_ids(self, project_id, &request)?;
        let current = self.get(project_id)?.revision;
        if request.expected_revision != current {
            return Err(ProjectError::RevisionConflict {
                expected: request.expected_revision,
                current,
            });
        }
        request
            .expected_revision
            .checked_add(request.commands.len() as u64)
            .ok_or(CommandBatchError::RevisionSpaceExhausted)?;

        let base_revision = request.expected_revision;
        let mut results = Vec::with_capacity(request.commands.len());
        for (index, command) in request.commands.into_iter().enumerate() {
            let result = self.execute(
                project_id,
                CommandRequest {
                    request_id: child_request_id(request.request_id, index),
                    expected_revision: base_revision + index as u64,
                    command,
                },
            )?;
            let entry = self
                .projects
                .get_mut(project_id)
                .expect("project was validated");
            let change = entry
                .changes
                .get_mut(&result.project_revision)
                .expect("new command appended its ChangeSet");
            change.batch_id = Some(request.request_id);
            change.compensates = compensates;
            results.push(result);
        }
        Ok(CommandBatchResult {
            request_id: request.request_id,
            base_revision,
            project_revision: results.last().unwrap().project_revision,
            compensates,
            results,
        })
    }
}

fn validate_child_ids(
    catalog: &ProjectCatalog,
    project: &ProjectId,
    request: &CommandBatchRequest,
) -> Result<(), ProjectError> {
    let entry = catalog
        .projects
        .get(project)
        .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
    if (0..request.commands.len())
        .map(|index| child_request_id(request.request_id, index))
        .any(|request_id| entry.results.contains_key(&request_id))
    {
        return Err(CommandBatchError::RequestConflict(request.request_id).into());
    }
    Ok(())
}

fn validate_batch(request: &CommandBatchRequest) -> Result<(), CommandBatchError> {
    if request.commands.is_empty() {
        return Err(CommandBatchError::Empty);
    }
    if request.commands.len() > MAX_COMMAND_BATCH_SIZE {
        return Err(CommandBatchError::TooLarge {
            count: request.commands.len(),
            maximum: MAX_COMMAND_BATCH_SIZE,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        command::{
            CommandBatchRequest, CommandRequest, CreateNode, GraphCommand, child_request_id,
        },
        domain::{Factor, NodePayload},
        project::{CommandBatchError, ProjectCatalog, ProjectError},
    };

    fn create(name: &str) -> GraphCommand {
        GraphCommand::CreateNode(CreateNode {
            name: name.to_owned(),
            title: name.to_owned(),
            payload: NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        })
    }

    #[test]
    fn rejects_a_batch_whose_child_id_was_used_by_another_command() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let batch_id = Uuid::new_v4();
        catalog
            .execute(
                &project.id,
                CommandRequest {
                    request_id: child_request_id(batch_id, 0),
                    expected_revision: 0,
                    command: create("existing"),
                },
            )
            .unwrap();
        let error = catalog
            .execute_batch(
                &project.id,
                CommandBatchRequest {
                    request_id: batch_id,
                    expected_revision: 1,
                    commands: vec![create("new")],
                },
                None,
            )
            .unwrap_err();
        assert_eq!(
            error,
            ProjectError::CommandBatch(CommandBatchError::RequestConflict(batch_id))
        );
        assert_eq!(catalog.list_nodes(&project.id).unwrap().len(), 1);
    }
}
