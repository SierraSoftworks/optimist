use crate::{
    command::{CommandOutcome, SetNodeQuantityState},
    domain::{NodePayload, QuantityState},
    store::{GraphRepository, RepositoryError},
};

use super::{AggregateUpdateError, ProjectError, catalog::ProjectEntry};

pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetNodeQuantityState,
) -> Result<CommandOutcome, ProjectError> {
    let mut node = entry
        .repository
        .get_node(command.node)?
        .ok_or(RepositoryError::MissingEntity(command.node))?;
    if node.revision != command.expected_revision {
        return Err(AggregateUpdateError::NodeRevisionConflict {
            id: node.id,
            expected: command.expected_revision,
            current: node.revision,
        }
        .into());
    }
    match &node.payload {
        NodePayload::Factor(_) | NodePayload::Outcome(_) => {}
        _ => return Err(ProjectError::NativeStateUnsupported(node.id)),
    }
    let native_is_empty = node
        .native_state
        .as_ref()
        .is_none_or(|state| state.current.is_none() && state.forecast.is_none());
    if !native_is_empty {
        return Err(ProjectError::StateEstimatesAlreadyExist(node.id));
    }
    node.native_state = Some(QuantityState::new(command.quantity, None, None)?);
    node.revision = node
        .revision
        .checked_add(1)
        .ok_or(AggregateUpdateError::NodeRevisionSpaceExhausted(node.id))?;
    entry.repository.update_node(node.clone())?;
    Ok(CommandOutcome::NodeQuantityStateSet(node))
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandRequest, CreateNode, GraphCommand, SetNodeQuantityState, SetSquiggleEstimate,
        },
        domain::{
            EntityId, EstimateAddress, EstimateId, EstimateOwner, EstimateSlot,
            EstimateUncertainty, Factor, NodePayload, QuantityDefinition, QuantitySupport, Unit,
        },
        project::{ProjectCatalog, ProjectError},
    };

    fn factor(name: &str) -> CreateNode {
        CreateNode {
            name: name.to_owned(),
            title: name.to_owned(),
            payload: NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        }
    }

    #[test]
    fn configures_native_state_and_validates_estimates() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Native".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("source"))),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                        node: EntityId::new(0),
                        expected_revision: 0,
                        quantity: QuantityDefinition::with_dimension(
                            "days",
                            Some(Unit::base("day").unwrap()),
                            None,
                            QuantitySupport::NonNegative,
                        )
                        .unwrap(),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::NodeQuantityStateSet(node) = result.outcome else {
            panic!("expected configured node")
        };
        assert_eq!(node.native_state.unwrap().quantity.unit, "days");

        let address = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        let created = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        definition: crate::domain::SquiggleEstimateDefinition {
                            source: "pointMass(12)".to_owned(),
                            seed: 42,
                            sample_count: 256,
                            target_unit: Unit::base("day").unwrap(),
                        },
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::SquiggleEstimateSet(created) = created.outcome else {
            panic!("expected native estimate")
        };
        assert_eq!(created.quantity.unwrap().unit, "days");

        let invalid = catalog.execute(
            &project.id,
            CommandRequest::new(
                3,
                GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                    address,
                    slot: EstimateSlot::Current,
                    definition: crate::domain::SquiggleEstimateDefinition {
                        source: "pointMass(-1)".to_owned(),
                        seed: 42,
                        sample_count: 256,
                        target_unit: Unit::base("day").unwrap(),
                    },
                    provenance: vec![],
                    uncertainty: EstimateUncertainty::default(),
                }),
            ),
        );
        assert!(matches!(invalid, Err(ProjectError::Quantity(_))));
    }
}
