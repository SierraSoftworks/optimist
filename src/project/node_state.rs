use crate::{
    command::{CommandOutcome, SetNodeQuantityState},
    domain::{EdgePayload, NodePayload, QuantityState},
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
    let legacy_is_empty = match &node.payload {
        NodePayload::Factor(value) => value.current.is_none() && value.desired.is_none(),
        NodePayload::Outcome(value) => value.current.is_none() && value.desired.is_none(),
        _ => return Err(ProjectError::NativeStateUnsupported(node.id)),
    };
    let native_is_empty = node
        .native_state
        .as_ref()
        .is_none_or(|state| state.current.is_none() && state.forecast.is_none());
    if !legacy_is_empty || !native_is_empty {
        return Err(ProjectError::StateEstimatesAlreadyExist(node.id));
    }
    if let Some(edge) = entry.repository.list_edges()?.into_iter().find(|edge| {
        (edge.source == node.id || edge.destination == node.id)
            && matches!(
                &edge.payload,
                EdgePayload::Contributes(effect) | EdgePayload::Changes(effect)
                    if effect.normalized_effect().is_some()
            )
    }) {
        return Err(ProjectError::NativeStateNormalizedEdge(edge.id()));
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
            CommandRequest, CreateEdge, CreateNode, GraphCommand, SetEstimate, SetNodeQuantityState,
        },
        domain::{
            CausalEffect, Distribution, EdgePayload, EntityId, Estimate, EstimateAddress,
            EstimateId, EstimateOwner, EstimateSlot, EstimateUncertainty, Factor, NodePayload,
            QuantityDefinition, QuantitySupport, SignedInfluence, Unit,
        },
        project::{ProjectCatalog, ProjectError},
    };

    fn factor(name: &str) -> CreateNode {
        CreateNode {
            name: name.to_owned(),
            title: name.to_owned(),
            payload: NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: false,
                evidence: vec![],
            }),
        }
    }

    #[test]
    fn configures_native_state_and_rejects_normalized_causal_edges() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Native".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("source"))),
            )
            .unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(1, GraphCommand::CreateNode(factor("target"))),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
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

        let effect =
            Estimate::<SignedInfluence>::new(EstimateId::new(0), Distribution::point(0.5).unwrap())
                .unwrap();
        let result = catalog.execute(
            &project.id,
            CommandRequest::new(
                3,
                GraphCommand::CreateEdge(CreateEdge {
                    source: EntityId::new(0),
                    destination: EntityId::new(1),
                    payload: EdgePayload::Contributes(CausalEffect::normalized(
                        effect,
                        None,
                        String::new(),
                        vec![],
                    )),
                }),
            ),
        );
        assert!(matches!(
            result,
            Err(ProjectError::NativeCausalResponseRequired(_))
        ));

        let address = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        let created = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    3,
                    GraphCommand::SetEstimate(SetEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        distribution: Distribution::point(12.0).unwrap(),
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::EstimateSet(created) = created.outcome else {
            panic!("expected native estimate")
        };
        assert_eq!(created.quantity.unwrap().unit, "days");

        let invalid = catalog.execute(
            &project.id,
            CommandRequest::new(
                4,
                GraphCommand::SetEstimate(SetEstimate {
                    address,
                    slot: EstimateSlot::Current,
                    distribution: Distribution::point(-1.0).unwrap(),
                    provenance: vec![],
                    uncertainty: EstimateUncertainty::default(),
                }),
            ),
        );
        assert!(matches!(invalid, Err(ProjectError::Quantity(_))));
    }
}
