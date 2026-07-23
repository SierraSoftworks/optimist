use crate::{
    command::{CommandOutcome, SetNodeQuantityState},
    domain::{
        EdgePayload, EstimateAddress, EstimateOwner, NodePayload, NormalizedState,
        QuantityDefinition, QuantityState,
    },
    store::{GraphRepository, RepositoryError},
};

use super::{
    AggregateUpdateError, EstimateCommandError, ProjectError, catalog::ProjectEntry,
    estimate_formula_references,
};

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
    let (legacy_current, legacy_desired) = match &node.payload {
        NodePayload::Factor(value) => (value.current.as_ref(), value.desired.as_ref()),
        NodePayload::Outcome(value) => (value.current.as_ref(), value.desired.as_ref()),
        _ => return Err(ProjectError::NativeStateUnsupported(node.id)),
    };
    let native_is_empty = node
        .native_state
        .as_ref()
        .is_none_or(|state| state.current.is_none() && state.forecast.is_none());
    if !native_is_empty {
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
    let has_legacy = legacy_current.is_some() || legacy_desired.is_some();
    let mapping = match (has_legacy, command.legacy_mapping) {
        (true, Some(mapping)) => Some(mapping.validated()?),
        (true, None) => return Err(ProjectError::LegacyStateMappingRequired(node.id)),
        (false, mapping) => mapping.map(|mapping| mapping.validated()).transpose()?,
    };
    if let Some(mapping) = mapping
        && !command.quantity.accepts(
            &crate::domain::Distribution::scaled_beta(
                1.0,
                1.0,
                mapping.state_zero,
                mapping.state_one,
            )
            .expect("validated mapping forms bounded support"),
        )
    {
        return Err(crate::domain::QuantityError::EstimateOutsideSupport.into());
    }
    if has_legacy {
        for estimate in [legacy_current, legacy_desired].into_iter().flatten() {
            let address = EstimateAddress::new(
                entry.project.id.clone(),
                EstimateOwner::Node(node.id),
                estimate.id,
            );
            if let Some(formula) = estimate_formula_references::find(entry, &address) {
                return Err(EstimateCommandError::ReferencedByFormula {
                    address,
                    formula: Box::new(formula),
                }
                .into());
            }
        }
    }
    let (current, desired) = take_legacy_state(&mut node.payload);
    let native_current = convert(current, &command.quantity, mapping)?;
    let native_forecast = convert(desired, &command.quantity, mapping)?;
    node.native_state = Some(QuantityState::new(
        command.quantity,
        native_current,
        native_forecast,
    )?);
    node.revision = node
        .revision
        .checked_add(1)
        .ok_or(AggregateUpdateError::NodeRevisionSpaceExhausted(node.id))?;
    entry.repository.update_node(node.clone())?;
    Ok(CommandOutcome::NodeQuantityStateSet(node))
}

fn take_legacy_state(
    payload: &mut NodePayload,
) -> (
    Option<crate::domain::Estimate<NormalizedState>>,
    Option<crate::domain::Estimate<NormalizedState>>,
) {
    match payload {
        NodePayload::Factor(value) => (value.current.take(), value.desired.take()),
        NodePayload::Outcome(value) => (value.current.take(), value.desired.take()),
        _ => unreachable!("native state kind checked"),
    }
}

fn convert(
    estimate: Option<crate::domain::Estimate<NormalizedState>>,
    quantity: &QuantityDefinition,
    mapping: Option<crate::domain::LegacyStateMapping>,
) -> Result<Option<crate::domain::Estimate<crate::domain::QuantityValue>>, ProjectError> {
    estimate
        .map(|estimate| {
            estimate
                .into_native_quantity(
                    quantity,
                    mapping.expect("populated legacy state has mapping"),
                )
                .map_err(EstimateCommandError::from)
                .map_err(ProjectError::from)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandRequest, CreateEdge, CreateNode, GraphCommand, SetEstimate, SetNodeQuantityState,
        },
        domain::{
            CausalEffect, Distribution, EdgePayload, EntityId, Estimate, EstimateAddress,
            EstimateId, EstimateOwner, EstimateSlot, EstimateUncertainty, Factor,
            LegacyStateMapping, NodePayload, NormalizedState, QuantityDefinition, QuantitySupport,
            SignedInfluence, SquiggleEstimateDefinition, Unit,
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
                        legacy_mapping: None,
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

    #[test]
    fn explicit_mapping_migrates_legacy_current_and_desired_state() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Migration".to_owned()).unwrap();
        let current = Estimate::<NormalizedState>::new(
            EstimateId::new(0),
            Distribution::beta(2.0, 2.0).unwrap(),
        )
        .unwrap();
        let desired = Estimate::<NormalizedState>::from_squiggle(
            EstimateId::new(1),
            SquiggleEstimateDefinition {
                source: "pointMass(0.25)".to_owned(),
                seed: 42,
                sample_count: 256,
                target_unit: Unit::dimensionless(),
            },
            &Unit::dimensionless(),
        )
        .unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "latency".to_owned(),
                        title: "Latency".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            current: Some(current),
                            desired: Some(desired),
                            controllable: false,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let quantity = QuantityDefinition::with_dimension(
            "minutes",
            Some(Unit::base("minute").unwrap()),
            None,
            QuantitySupport::Bounded {
                lower: 100.0,
                upper: 200.0,
            },
        )
        .unwrap();
        let without_mapping = catalog.execute(
            &project.id,
            CommandRequest::new(
                1,
                GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                    node: EntityId::new(0),
                    expected_revision: 0,
                    quantity: quantity.clone(),
                    legacy_mapping: None,
                }),
            ),
        );
        assert!(matches!(
            without_mapping,
            Err(ProjectError::LegacyStateMappingRequired(_))
        ));

        let outside_support = catalog.execute(
            &project.id,
            CommandRequest::new(
                1,
                GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                    node: EntityId::new(0),
                    expected_revision: 0,
                    quantity: quantity.clone(),
                    legacy_mapping: Some(LegacyStateMapping {
                        state_zero: 50.0,
                        state_one: 200.0,
                    }),
                }),
            ),
        );
        assert!(matches!(outside_support, Err(ProjectError::Quantity(_))));

        let result = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                        node: EntityId::new(0),
                        expected_revision: 0,
                        quantity,
                        legacy_mapping: Some(LegacyStateMapping {
                            state_zero: 100.0,
                            state_one: 200.0,
                        }),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::NodeQuantityStateSet(node) = result.outcome else {
            panic!("expected migrated node")
        };
        let NodePayload::Factor(legacy) = node.payload else {
            panic!("expected factor")
        };
        assert!(legacy.current.is_none() && legacy.desired.is_none());
        let native = node.native_state.unwrap();
        assert_eq!(native.current.unwrap().distribution.mean(), 150.0);
        assert_eq!(native.forecast.unwrap().distribution.mean(), 125.0);
    }
}
