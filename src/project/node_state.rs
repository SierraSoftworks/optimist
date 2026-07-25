use std::collections::BTreeMap;

use crate::{
    command::{CommandOutcome, SetNodeQuantityState},
    domain::{Node, NodePayload, QuantityState, state_relation_schema},
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
    match &mut node.payload {
        NodePayload::Metric(metric) => {
            *metric = metric.clone().with_quantity_replacement(command.quantity)?;
        }
        NodePayload::Factor(_) | NodePayload::Outcome(_) => {
            node.native_state = Some(match node.native_state.take() {
                Some(state) => state.with_quantity(command.quantity)?,
                None => QuantityState::new(command.quantity, None, None)?,
            });
        }
        _ => return Err(ProjectError::NativeStateUnsupported(node.id)),
    }
    node.revision = node
        .revision
        .checked_add(1)
        .ok_or(AggregateUpdateError::NodeRevisionSpaceExhausted(node.id))?;
    revalidate_relations(entry, &node)?;
    entry.repository.update_node(node.clone())?;
    Ok(CommandOutcome::NodeQuantityStateSet(node))
}

/// Rejects a unit change that would break an equation reading this quantity.
///
/// Relationships stopped carrying units when responses became dimensionless, so
/// a causal edge no longer constrains its endpoints. Node equations do: they are
/// checked against the units their parents declare, and this node may be a
/// parent of several. Recompiling them here keeps the failure beside the edit
/// that caused it rather than surfacing as a broken projection.
fn revalidate_relations(entry: &mut ProjectEntry, updated: &Node) -> Result<(), ProjectError> {
    let edges = entry.repository.list_edges()?;
    let nodes = entry
        .repository
        .list_nodes()?
        .into_iter()
        .map(|node| {
            if node.id == updated.id {
                updated.clone()
            } else {
                node
            }
        })
        .collect::<Vec<_>>();
    let by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
    for node in &nodes {
        let Some(relation) = state_relation_schema::relation_of(node) else {
            continue;
        };
        state_relation_schema::compile(node, &by_id, &edges, relation).map_err(|error| {
            ProjectError::StateQuantityBreaksRelation {
                node: node.id,
                reason: error.to_string(),
            }
        })?;
    }
    Ok(())
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
        project::{EstimateCommandError, ProjectCatalog, ProjectError},
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
        assert!(matches!(
            invalid,
            Err(ProjectError::EstimateCommand(
                EstimateCommandError::IncompatibleSupport { .. }
            ))
        ));
    }

    #[test]
    fn edits_native_quantity_type_and_revalidates_existing_squiggle() {
        let unit = Unit::from_exponents([("change", 1), ("month", -1)]).unwrap();
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Native".to_owned()).unwrap();
        catalog
            .execute(
                &project.id,
                CommandRequest::new(0, GraphCommand::CreateNode(factor("frequency"))),
            )
            .unwrap();
        let quantity = |support| {
            QuantityDefinition::with_dimension(
                "changes/month",
                Some(unit.clone()),
                Some("total monthly".to_owned()),
                support,
            )
            .unwrap()
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    1,
                    GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                        node: EntityId::new(0),
                        expected_revision: 0,
                        quantity: quantity(QuantitySupport::NonNegative),
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
                    2,
                    GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                        address: address.clone(),
                        slot: EstimateSlot::Current,
                        definition: crate::domain::SquiggleEstimateDefinition {
                            source: "changesPerMonth :: change/month = lognormal(5, 0.4)\nchangesPerMonth".to_owned(),
                            seed: 42,
                            sample_count: 256,
                            target_unit: unit.clone(),
                        },
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();

        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    3,
                    GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                        node: EntityId::new(0),
                        expected_revision: 2,
                        quantity: quantity(QuantitySupport::Real),
                    }),
                ),
            )
            .unwrap();
        let source =
            "changesPerMonth :: change/month = normal({ p10: 50, p90: 500 })\nchangesPerMonth";
        let result = catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    4,
                    GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                        address,
                        slot: EstimateSlot::Current,
                        definition: crate::domain::SquiggleEstimateDefinition {
                            source: source.to_owned(),
                            seed: 42,
                            sample_count: 256,
                            target_unit: unit,
                        },
                        provenance: vec![],
                        uncertainty: EstimateUncertainty::default(),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::SquiggleEstimateSet(estimate) = result.outcome else {
            panic!("expected updated estimate")
        };
        let crate::domain::EstimateSource::Squiggle { definition } = estimate.source;
        assert_eq!(definition.source, source);
    }

    /// A relationship no longer constrains its endpoints, but an equation does.
    #[test]
    fn rejects_a_unit_change_that_would_break_a_downstream_equation() {
        use crate::command::{CreateEdge, SetStateRelation};
        use crate::domain::{
            CausalEffect, Distribution, EdgePayload, Elasticity, Estimate, Metric, Outcome,
            OutcomeDirection, StateRelation,
        };

        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Equations".to_owned()).unwrap().id;
        let mut revision = 0;
        let mut execute = |catalog: &mut ProjectCatalog, command| {
            let result = catalog
                .execute(&project, CommandRequest::new(revision, command))
                .unwrap();
            revision = result.project_revision;
        };
        execute(
            &mut catalog,
            GraphCommand::CreateNode(CreateNode {
                name: "outage_frequency".to_owned(),
                title: "Outage frequency".to_owned(),
                payload: NodePayload::Metric(
                    Metric::with_quantity(
                        QuantityDefinition::with_dimension(
                            "outages",
                            Some(Unit::base("outage").unwrap()),
                            None,
                            QuantitySupport::NonNegative,
                        )
                        .unwrap(),
                        Some(
                            Estimate::new(EstimateId::new(0), Distribution::point(4.0).unwrap())
                                .unwrap(),
                        ),
                    )
                    .unwrap(),
                ),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::CreateNode(CreateNode {
                name: "impact".to_owned(),
                title: "Impact".to_owned(),
                payload: NodePayload::Outcome(Outcome {
                    direction: OutcomeDirection::Minimize,
                    evidence: vec![],
                }),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                node: EntityId::new(1),
                expected_revision: 0,
                quantity: QuantityDefinition::with_dimension(
                    "outages",
                    Some(Unit::base("outage").unwrap()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::CreateEdge(CreateEdge {
                source: EntityId::new(0),
                destination: EntityId::new(1),
                payload: EdgePayload::Contributes(CausalEffect::proportional(
                    Estimate::<Elasticity>::new(
                        EstimateId::new(0),
                        Distribution::point(1.0).unwrap(),
                    )
                    .unwrap(),
                    None,
                    String::new(),
                    vec![],
                )),
            }),
        );
        execute(
            &mut catalog,
            GraphCommand::SetStateRelation(SetStateRelation {
                node: EntityId::new(1),
                expected_revision: 1,
                relation: Some(
                    StateRelation::new("outage_frequency".to_owned(), Default::default()).unwrap(),
                ),
            }),
        );

        // Re-uniting the parent leaves the equation producing outages while its
        // owner now expects minutes.
        let broken = catalog.execute(
            &project,
            CommandRequest::new(
                revision,
                GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                    node: EntityId::new(1),
                    expected_revision: 2,
                    quantity: QuantityDefinition::with_dimension(
                        "minutes",
                        Some(Unit::base("minute").unwrap()),
                        None,
                        QuantitySupport::NonNegative,
                    )
                    .unwrap(),
                }),
            ),
        );
        assert!(matches!(
            broken,
            Err(ProjectError::StateQuantityBreaksRelation { .. })
        ));
    }

    /// A metric's unit is a modelling decision that can turn out to be wrong.
    #[test]
    fn moves_a_metric_onto_a_corrected_unit_and_reassesses_its_estimate() {
        use crate::domain::{Distribution, Estimate, Metric};

        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Metrics".to_owned()).unwrap().id;
        catalog
            .execute(
                &project,
                CommandRequest::new(
                    0,
                    GraphCommand::CreateNode(CreateNode {
                        name: "ttm".to_owned(),
                        title: "Time to mitigate".to_owned(),
                        payload: NodePayload::Metric(
                            Metric::with_quantity(
                                QuantityDefinition::with_dimension(
                                    "minutes",
                                    Some(Unit::base("minute").unwrap()),
                                    None,
                                    QuantitySupport::NonNegative,
                                )
                                .unwrap(),
                                Some(
                                    Estimate::new(
                                        EstimateId::new(0),
                                        Distribution::point(90.0).unwrap(),
                                    )
                                    .unwrap(),
                                ),
                            )
                            .unwrap(),
                        ),
                    }),
                ),
            )
            .unwrap();
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    1,
                    GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                        node: EntityId::new(0),
                        expected_revision: 0,
                        quantity: QuantityDefinition::with_dimension(
                            "minutes/outage",
                            Some(Unit::from_exponents([("minute", 1), ("outage", -1)]).unwrap()),
                            Some("mean per incident".to_owned()),
                            QuantitySupport::NonNegative,
                        )
                        .unwrap(),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::NodeQuantityStateSet(node) = result.outcome else {
            panic!("expected a retargeted metric")
        };
        let NodePayload::Metric(metric) = node.payload else {
            panic!("expected a metric payload")
        };
        assert_eq!(metric.quantity.unit, "minutes/outage");
        let estimate = metric.current.expect("the estimate survives the move");
        let crate::domain::EstimateSource::Squiggle { definition } = estimate.source;
        assert_eq!(
            definition.target_unit,
            Unit::from_exponents([("minute", 1), ("outage", -1)]).unwrap(),
            "the stored source must be reassessed against the corrected unit"
        );
        assert_eq!(estimate.distribution.mean(), 90.0);
    }
}
