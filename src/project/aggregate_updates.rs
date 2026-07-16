use crate::{
    command::{CommandOutcome, UpdateEdgeMetadata, UpdateNodeMetadata},
    store::{GraphRepository, RepositoryError},
};

use super::{AggregateUpdateError, ProjectError, catalog::ProjectEntry};

pub(super) fn node(
    entry: &mut ProjectEntry,
    command: UpdateNodeMetadata,
) -> Result<CommandOutcome, ProjectError> {
    if command.title.trim().is_empty() {
        return Err(crate::domain::NodeError::EmptyTitle.into());
    }
    let mut node = entry
        .repository
        .get_node(command.id)?
        .ok_or(RepositoryError::MissingEntity(command.id))?;
    if node.revision != command.expected_revision {
        return Err(AggregateUpdateError::NodeRevisionConflict {
            id: command.id,
            expected: command.expected_revision,
            current: node.revision,
        }
        .into());
    }
    node.revision = node
        .revision
        .checked_add(1)
        .ok_or(AggregateUpdateError::NodeRevisionSpaceExhausted(command.id))?;
    node.title = command.title;
    node.description = command.description;
    node.metadata = command.metadata;
    entry.repository.update_node_metadata(node.clone())?;
    Ok(CommandOutcome::NodeMetadataUpdated(node))
}

pub(super) fn edge(
    entry: &mut ProjectEntry,
    command: UpdateEdgeMetadata,
) -> Result<CommandOutcome, ProjectError> {
    let mut edge = entry
        .repository
        .get_edge(&command.id)?
        .ok_or_else(|| RepositoryError::MissingEdge(command.id.to_string()))?;
    if edge.revision != command.expected_revision {
        return Err(AggregateUpdateError::EdgeRevisionConflict {
            id: command.id,
            expected: command.expected_revision,
            current: edge.revision,
        }
        .into());
    }
    edge.revision = edge
        .revision
        .checked_add(1)
        .ok_or_else(|| ProjectError::EdgeRevisionSpaceExhausted(command.id.clone()))?;
    edge.description = command.description;
    edge.metadata = command.metadata;
    entry.repository.update_edge(edge.clone())?;
    Ok(CommandOutcome::EdgeMetadataUpdated(edge))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateEdge, CreateNode, GraphCommand,
            UpdateEdgeMetadata, UpdateNodeMetadata,
        },
        domain::{EdgePayload, EntityId, Factor, NodePayload, Requirement},
        project::{AggregateUpdateError, ProjectCatalog, ProjectError},
    };

    fn catalog() -> (ProjectCatalog, crate::domain::ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        for (revision, name) in [(0, "left"), (1, "right")] {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::CreateNode(CreateNode {
                            name: name.to_owned(),
                            title: name.to_owned(),
                            payload: NodePayload::Factor(Factor {
                                current: None,
                                desired: None,
                                controllable: true,
                                evidence: vec![],
                            }),
                        }),
                    ),
                )
                .unwrap();
        }
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: EntityId::new(0),
                        destination: EntityId::new(1),
                        payload: EdgePayload::Requires(Requirement {
                            hard: true,
                            satisfaction_threshold: None,
                        }),
                    }),
                ),
            )
            .unwrap();
        (catalog, project.id)
    }

    #[test]
    fn updates_node_metadata_with_revision_and_replay_guards() {
        let (mut catalog, project) = catalog();
        let request = CommandRequest::new(
            3,
            GraphCommand::UpdateNodeMetadata(UpdateNodeMetadata {
                id: EntityId::new(0),
                expected_revision: 0,
                title: "Delivery flow".to_owned(),
                description: "# Flow\n\nBoundary.".to_owned(),
                metadata: BTreeMap::from([("owner".to_owned(), json!("team"))]),
            }),
        );
        let first = catalog.execute(&project, request.clone()).unwrap();
        assert_eq!(first, catalog.execute(&project, request).unwrap());
        let CommandOutcome::NodeMetadataUpdated(node) = first.outcome else {
            unreachable!()
        };
        assert_eq!(node.revision, 1);
        assert_eq!(node.title, "Delivery flow");
        assert!(matches!(node.payload, NodePayload::Factor(_)));
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    4,
                    GraphCommand::UpdateNodeMetadata(UpdateNodeMetadata {
                        id: node.id,
                        expected_revision: 0,
                        title: "stale".to_owned(),
                        description: String::new(),
                        metadata: BTreeMap::new(),
                    }),
                ),
            ),
            Err(ProjectError::AggregateUpdate(
                AggregateUpdateError::NodeRevisionConflict { current: 1, .. }
            ))
        ));
    }

    #[test]
    fn updates_edge_description_and_metadata_without_changing_payload() {
        let (mut catalog, project) = catalog();
        let edge = catalog.list_edges(&project).unwrap().remove(0);
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::UpdateEdgeMetadata(UpdateEdgeMetadata {
                        id: edge.id(),
                        expected_revision: 0,
                        description: "# Dependency\n\nRequired first.".to_owned(),
                        metadata: BTreeMap::from([("source".to_owned(), json!("ADR-1"))]),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EdgeMetadataUpdated(updated) = result.outcome else {
            unreachable!()
        };
        assert_eq!(updated.revision, 1);
        assert_eq!(updated.description, "# Dependency\n\nRequired first.");
        assert_eq!(updated.payload, edge.payload);
        assert_eq!(updated.id(), edge.id());
    }
}
