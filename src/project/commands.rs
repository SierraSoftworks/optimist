use crate::{
    command::{CommandOutcome, CommandRequest, CommandResult, GraphCommand},
    domain::{EntityId, Node, ProjectId},
    store::GraphRepository,
};

use super::{ProjectCatalog, ProjectError};

impl ProjectCatalog {
    /// Applies a typed graph command under project revision and idempotency checks.
    ///
    /// A duplicate request ID returns its original result before comparing revisions.
    /// New commands must match the current revision and are serialized by the mutable
    /// catalog borrow used by the server's per-project write path.
    pub fn execute(
        &mut self,
        project_id: &ProjectId,
        request: CommandRequest,
    ) -> Result<CommandResult, ProjectError> {
        let entry = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| ProjectError::NotFound(project_id.clone()))?;
        if let Some(result) = entry.results.get(&request.request_id) {
            return Ok(result.clone());
        }
        if request.expected_revision != entry.project.revision {
            return Err(ProjectError::RevisionConflict {
                expected: request.expected_revision,
                current: entry.project.revision,
            });
        }

        let outcome = match request.command {
            GraphCommand::CreateNode(command) => {
                let id = entry.repository.next_entity_id()?;
                let node = Node::new(id, command.name, command.title, command.payload)?;
                entry.repository.create_node(node.clone())?;
                CommandOutcome::NodeCreated(node)
            }
        };
        entry.project.revision = entry
            .project
            .revision
            .checked_add(1)
            .ok_or_else(|| ProjectError::RevisionSpaceExhausted(project_id.clone()))?;
        let result = CommandResult {
            request_id: request.request_id,
            project_revision: entry.project.revision,
            outcome,
        };
        entry.results.insert(request.request_id, result.clone());
        Ok(result)
    }

    /// Lists complete node aggregates for one project in deterministic ID order.
    pub fn list_nodes(&mut self, project_id: &ProjectId) -> Result<Vec<Node>, ProjectError> {
        Ok(self.repository_mut(project_id)?.list_nodes()?)
    }

    /// Retrieves one complete node aggregate from a project-local entity ID.
    pub fn get_node(
        &mut self,
        project_id: &ProjectId,
        entity_id: EntityId,
    ) -> Result<Option<Node>, ProjectError> {
        Ok(self.repository_mut(project_id)?.get_node(entity_id)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{CommandOutcome, CommandRequest, CreateNode, GraphCommand},
        domain::{Factor, NodePayload},
    };

    use super::ProjectCatalog;
    use crate::project::ProjectError;

    fn create_node(revision: u64) -> CommandRequest {
        CommandRequest::new(
            revision,
            GraphCommand::CreateNode(CreateNode {
                name: "github".to_owned(),
                title: "GitHub".to_owned(),
                payload: NodePayload::Factor(Factor {
                    current: None,
                    desired: None,
                    controllable: false,
                    evidence: vec![],
                }),
            }),
        )
    }

    #[test]
    fn applies_commands_idempotently_and_advances_revision() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        let request = create_node(0);
        let first = catalog.execute(&project.id, request.clone()).unwrap();
        let retry = catalog.execute(&project.id, request).unwrap();

        assert_eq!(first, retry);
        assert_eq!(first.project_revision, 1);
        assert!(matches!(first.outcome, CommandOutcome::NodeCreated(_)));
        assert_eq!(catalog.list_nodes(&project.id).unwrap().len(), 1);
    }

    #[test]
    fn rejects_stale_commands_without_mutating_graph() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog.execute(&project.id, create_node(0)).unwrap();
        let error = catalog.execute(&project.id, create_node(0)).unwrap_err();

        assert!(matches!(
            error,
            ProjectError::RevisionConflict { current: 1, .. }
        ));
        assert_eq!(catalog.list_nodes(&project.id).unwrap().len(), 1);
    }
}
