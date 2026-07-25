use std::collections::BTreeMap;

use crate::{
    command::{CommandOutcome, SetStateRelation},
    domain::{Node, NodePayload, StateRelation, state_relation_schema},
    store::{GraphRepository, RepositoryError},
};

use super::{AggregateUpdateError, ProjectError, catalog::ProjectEntry};

/// Attaches or clears the equation computing one state's value each period.
///
/// The equation is compiled here rather than only at analysis time, because the
/// names it may bind and the units they carry are properties of the surrounding
/// graph. Rejecting it now turns a modelling mistake into an error beside the
/// edit that caused it instead of a projection that fails much later.
pub(super) fn set(
    entry: &mut ProjectEntry,
    command: SetStateRelation,
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
    if let Some(relation) = &command.relation {
        let nodes = entry.repository.list_nodes()?;
        let edges = entry.repository.list_edges()?;
        let by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        let owner = by_id
            .get(&node.id)
            .copied()
            .ok_or(RepositoryError::MissingEntity(node.id))?;
        state_relation_schema::compile(owner, &by_id, &edges, relation)
            .map_err(ProjectError::StateRelation)?;
    }
    attach(&mut node, command.relation)?;
    node.revision = node
        .revision
        .checked_add(1)
        .ok_or(AggregateUpdateError::NodeRevisionSpaceExhausted(node.id))?;
    entry.repository.update_node(node.clone())?;
    Ok(CommandOutcome::StateRelationSet(node))
}

/// Stores the equation beside whichever quantity the node kind owns.
fn attach(node: &mut Node, relation: Option<StateRelation>) -> Result<(), ProjectError> {
    match &mut node.payload {
        NodePayload::Metric(metric) => {
            metric.relation = relation;
            Ok(())
        }
        NodePayload::Factor(_) | NodePayload::Outcome(_) => {
            let state = node
                .native_state
                .as_mut()
                .ok_or(ProjectError::NativeStateUnsupported(node.id))?;
            state.relation = relation;
            Ok(())
        }
        _ => Err(ProjectError::NativeStateUnsupported(node.id)),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{CommandOutcome, CommandRequest, CreateEdge, CreateNode, GraphCommand},
        domain::{
            CausalEffect, Distribution, EdgePayload, Elasticity, EntityId, Estimate, EstimateId,
            Metric, NodePayload, Outcome, OutcomeDirection, QuantityDefinition, QuantitySupport,
            StateRelation, Unit,
        },
        project::{ProjectCatalog, ProjectError},
    };

    /// Builds a project whose outcome is fed by two measured parents.
    fn fixture() -> (ProjectCatalog, crate::domain::ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Equations".to_owned()).unwrap().id;
        let mut revision = 0;
        let mut execute = |catalog: &mut ProjectCatalog, command| {
            let result = catalog
                .execute(&project, CommandRequest::new(revision, command))
                .unwrap();
            revision = result.project_revision;
        };
        let metric = |name: &str, unit: Unit, value: f64| {
            GraphCommand::CreateNode(CreateNode {
                name: name.to_owned(),
                title: name.to_owned(),
                payload: NodePayload::Metric(
                    Metric::with_quantity(
                        QuantityDefinition::with_dimension(
                            name,
                            Some(unit),
                            None,
                            QuantitySupport::NonNegative,
                        )
                        .unwrap(),
                        Some(
                            Estimate::new(EstimateId::new(0), Distribution::point(value).unwrap())
                                .unwrap(),
                        ),
                    )
                    .unwrap(),
                ),
            })
        };
        execute(
            &mut catalog,
            metric("outage_frequency", Unit::base("outage").unwrap(), 4.0),
        );
        execute(
            &mut catalog,
            metric(
                "impact_duration",
                Unit::from_exponents([("minute", 1), ("outage", -1)]).unwrap(),
                30.0,
            ),
        );
        let mut outcome = CreateNode {
            name: "customer_impact".to_owned(),
            title: "Customer impact".to_owned(),
            payload: NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Minimize,
                evidence: vec![],
            }),
        };
        outcome.payload = NodePayload::Outcome(Outcome {
            direction: OutcomeDirection::Minimize,
            evidence: vec![],
        });
        execute(&mut catalog, GraphCommand::CreateNode(outcome));
        execute(
            &mut catalog,
            GraphCommand::SetNodeQuantityState(crate::command::SetNodeQuantityState {
                node: EntityId::new(2),
                expected_revision: 0,
                quantity: QuantityDefinition::with_dimension(
                    "minutes",
                    Some(Unit::base("minute").unwrap()),
                    None,
                    QuantitySupport::NonNegative,
                )
                .unwrap(),
            }),
        );
        for source in [0, 1] {
            execute(
                &mut catalog,
                GraphCommand::CreateEdge(CreateEdge {
                    source: EntityId::new(source),
                    destination: EntityId::new(2),
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
        }
        (catalog, project)
    }

    fn set(source: &str) -> GraphCommand {
        GraphCommand::SetStateRelation(crate::command::SetStateRelation {
            node: EntityId::new(2),
            expected_revision: 1,
            relation: Some(StateRelation::new(source.to_owned(), Default::default()).unwrap()),
        })
    }

    #[test]
    fn attaches_an_equation_over_the_parents_the_graph_provides() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(revision, set("outage_frequency * impact_duration")),
            )
            .unwrap();
        let CommandOutcome::StateRelationSet(node) = result.outcome else {
            panic!("expected a node carrying its equation")
        };
        assert_eq!(
            node.native_state.unwrap().relation.unwrap().source,
            "outage_frequency * impact_duration"
        );
    }

    #[test]
    fn rejects_arithmetic_whose_units_do_not_produce_the_owning_quantity() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(revision, set("outage_frequency + impact_duration")),
            ),
            Err(ProjectError::StateRelation(_))
        ));
    }

    #[test]
    fn rejects_names_the_graph_does_not_connect_to_this_node() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(revision, set("outage_frequency * unrelated_factor")),
            ),
            Err(ProjectError::StateRelation(_))
        ));
    }

    #[test]
    fn rejects_uncertainty_authored_inside_the_equation() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        assert!(matches!(
            catalog.execute(
                &project,
                CommandRequest::new(
                    revision,
                    set("outage_frequency * impact_duration * normal(1, 0.1)"),
                ),
            ),
            Err(ProjectError::StateRelation(_))
        ));
    }

    #[test]
    fn clears_an_equation_and_restores_proportional_composition() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        catalog
            .execute(
                &project,
                CommandRequest::new(revision, set("outage_frequency * impact_duration")),
            )
            .unwrap();
        let revision = catalog.get(&project).unwrap().revision;
        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    revision,
                    GraphCommand::SetStateRelation(crate::command::SetStateRelation {
                        node: EntityId::new(2),
                        expected_revision: 2,
                        relation: None,
                    }),
                ),
            )
            .unwrap();
        let CommandOutcome::StateRelationSet(node) = result.outcome else {
            panic!("expected a node without its equation")
        };
        assert!(node.native_state.unwrap().relation.is_none());
    }

    /// A relation is stored as authored, so reloading a project must not compile it.
    #[test]
    fn survives_a_json_round_trip_without_graph_context() {
        let (mut catalog, project) = fixture();
        let revision = catalog.get(&project).unwrap().revision;
        catalog
            .execute(
                &project,
                CommandRequest::new(revision, set("outage_frequency * impact_duration")),
            )
            .unwrap();
        let node = catalog
            .get_node(&project, EntityId::new(2))
            .unwrap()
            .expect("the outcome exists");
        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(
            serde_json::from_value::<crate::domain::Node>(json).unwrap(),
            node
        );
    }
}
