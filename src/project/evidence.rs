use crate::{
    command::{CommandOutcome, CreateEvidence, DeleteEvidence, UpdateEvidence},
    domain::{Evidence, Node, NodePayload},
    store::{GraphRepository, RepositoryError},
};

use super::{EvidenceCommandError, ProjectError, catalog::ProjectEntry};

pub(super) fn create(
    entry: &mut ProjectEntry,
    command: CreateEvidence,
) -> Result<CommandOutcome, ProjectError> {
    let mut node = node(entry, command.node)?;
    let owner_next = node
        .revision
        .checked_add(1)
        .ok_or(EvidenceCommandError::IdentifierSpaceExhausted(node.id))?;
    let evidence_next = evidence_mut(&mut node)?
        .iter()
        .map(|value| value.id)
        .max()
        .map_or(Ok(0), |id| {
            id.checked_add(1)
                .ok_or(EvidenceCommandError::IdentifierSpaceExhausted(node.id))
        })?;
    let id = owner_next.max(evidence_next);
    let evidence = Evidence {
        id,
        revision: 0,
        summary: summary(command.summary)?,
        source: source(command.source),
    };
    evidence_mut(&mut node)?.push(evidence.clone());
    persist(entry, &mut node)?;
    Ok(CommandOutcome::EvidenceCreated { node, evidence })
}

pub(super) fn update(
    entry: &mut ProjectEntry,
    command: UpdateEvidence,
) -> Result<CommandOutcome, ProjectError> {
    let mut node = node(entry, command.node)?;
    let evidence = evidence_mut(&mut node)?
        .iter_mut()
        .find(|value| value.id == command.evidence_id)
        .ok_or(EvidenceCommandError::NotFound {
            node: command.node,
            evidence_id: command.evidence_id,
        })?;
    revision(command.node, evidence, command.expected_revision)?;
    evidence.summary = summary(command.summary)?;
    evidence.source = source(command.source);
    let evidence = evidence.clone();
    persist(entry, &mut node)?;
    Ok(CommandOutcome::EvidenceUpdated { node, evidence })
}

pub(super) fn delete(
    entry: &mut ProjectEntry,
    command: DeleteEvidence,
) -> Result<CommandOutcome, ProjectError> {
    let mut node = node(entry, command.node)?;
    let values = evidence_mut(&mut node)?;
    let index = values
        .iter()
        .position(|value| value.id == command.evidence_id)
        .ok_or(EvidenceCommandError::NotFound {
            node: command.node,
            evidence_id: command.evidence_id,
        })?;
    if values[index].revision != command.expected_revision {
        return Err(EvidenceCommandError::RevisionConflict {
            node: command.node,
            evidence_id: command.evidence_id,
            expected: command.expected_revision,
            current: values[index].revision,
        }
        .into());
    }
    let evidence = values.remove(index);
    persist(entry, &mut node)?;
    Ok(CommandOutcome::EvidenceDeleted { node, evidence })
}

fn node(entry: &ProjectEntry, id: crate::domain::EntityId) -> Result<Node, ProjectError> {
    entry
        .repository
        .get_node(id)?
        .ok_or_else(|| RepositoryError::MissingEntity(id).into())
}

fn evidence_mut(node: &mut Node) -> Result<&mut Vec<Evidence>, EvidenceCommandError> {
    match &mut node.payload {
        NodePayload::Factor(value) => Ok(&mut value.evidence),
        NodePayload::Outcome(value) => Ok(&mut value.evidence),
        _ => Err(EvidenceCommandError::InvalidOwner(node.id)),
    }
}

fn summary(value: String) -> Result<String, EvidenceCommandError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(EvidenceCommandError::EmptySummary);
    }
    Ok(value)
}

fn source(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_owned();
        (!value.is_empty()).then_some(value)
    })
}

fn revision(
    node: crate::domain::EntityId,
    evidence: &mut Evidence,
    expected: u64,
) -> Result<(), EvidenceCommandError> {
    if evidence.revision != expected {
        return Err(EvidenceCommandError::RevisionConflict {
            node,
            evidence_id: evidence.id,
            expected,
            current: evidence.revision,
        });
    }
    evidence.revision =
        evidence
            .revision
            .checked_add(1)
            .ok_or(EvidenceCommandError::RevisionSpaceExhausted {
                node,
                evidence_id: evidence.id,
            })?;
    Ok(())
}

fn persist(entry: &mut ProjectEntry, node: &mut Node) -> Result<(), ProjectError> {
    node.revision = node.revision.checked_add(1).ok_or(
        super::AggregateUpdateError::NodeRevisionSpaceExhausted(node.id),
    )?;
    entry.repository.update_node(node.clone())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandOutcome, CommandRequest, CreateEvidence, CreateNode, DeleteEvidence,
            GraphCommand, UpdateEvidence,
        },
        domain::{EntityId, Factor, Metric, NodePayload},
        project::{EvidenceCommandError, ProjectCatalog, ProjectError},
    };

    fn catalog(payload: NodePayload) -> (ProjectCatalog, crate::domain::ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "flow".to_owned(),
                        title: "Flow".to_owned(),
                        payload,
                    }),
                ),
            )
            .unwrap();
        (catalog, project.id)
    }

    #[test]
    fn creates_updates_and_deletes_evidence_with_independent_revisions() {
        let (mut catalog, project) = catalog(NodePayload::Factor(Factor {
            current: None,
            desired: None,
            controllable: false,
            evidence: vec![],
        }));
        let created = catalog
            .execute(
                &project,
                CommandRequest::new(
                    1,
                    GraphCommand::CreateEvidence(CreateEvidence {
                        node: EntityId::new(0),
                        summary: "  Queueing observed  ".to_owned(),
                        source: Some("  dashboard  ".to_owned()),
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EvidenceCreated { node, evidence } = created.outcome else {
            panic!("expected evidence creation")
        };
        assert_eq!((evidence.id, evidence.revision), (1, 0));
        assert_eq!(evidence.summary, "Queueing observed");
        assert_eq!(node.revision, 1);

        let updated = catalog
            .execute(
                &project,
                CommandRequest::new(
                    2,
                    GraphCommand::UpdateEvidence(UpdateEvidence {
                        node: EntityId::new(0),
                        evidence_id: evidence.id,
                        expected_revision: evidence.revision,
                        summary: "Queueing confirmed".to_owned(),
                        source: None,
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EvidenceUpdated { node, evidence } = updated.outcome else {
            panic!("expected evidence update")
        };
        assert_eq!(evidence.revision, 1);
        assert_eq!(node.revision, 2);

        let stale = catalog.execute(
            &project,
            CommandRequest::new(
                3,
                GraphCommand::DeleteEvidence(DeleteEvidence {
                    node: EntityId::new(0),
                    evidence_id: evidence.id,
                    expected_revision: 0,
                }),
            ),
        );
        assert!(matches!(
            stale,
            Err(ProjectError::EvidenceCommand(
                EvidenceCommandError::RevisionConflict { .. }
            ))
        ));

        let deleted = catalog
            .execute(
                &project,
                CommandRequest::new(
                    3,
                    GraphCommand::DeleteEvidence(DeleteEvidence {
                        node: EntityId::new(0),
                        evidence_id: evidence.id,
                        expected_revision: evidence.revision,
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::EvidenceDeleted {
            node,
            evidence: removed,
        } = deleted.outcome
        else {
            panic!("expected evidence deletion")
        };
        assert_eq!(removed, evidence);
        assert_eq!(node.revision, 3);
    }

    #[test]
    fn rejects_non_evidence_owners_and_empty_summaries() {
        let (mut catalog, project) =
            catalog(NodePayload::Metric(Metric::new("days", None).unwrap()));
        let invalid_owner = catalog.execute(
            &project,
            CommandRequest::new(
                1,
                GraphCommand::CreateEvidence(CreateEvidence {
                    node: EntityId::new(0),
                    summary: "Observed".to_owned(),
                    source: None,
                }),
            ),
        );
        assert_eq!(
            invalid_owner,
            Err(ProjectError::EvidenceCommand(
                EvidenceCommandError::InvalidOwner(EntityId::new(0))
            ))
        );
    }
}
