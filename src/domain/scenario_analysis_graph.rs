use std::collections::BTreeMap;

use super::{
    Edge, EdgePayload, EntityId, Intervention, Node, NodePayload, Scenario, ScenarioAnalysisError,
    scenario_analysis_edges::{self, InterventionEdge, PropagationEdge},
    scenario_analysis_reachability,
    scenario_analysis_state::{self, StateNode},
};

pub(super) struct AnalysisGraph<'a> {
    pub(super) states: Vec<StateNode>,
    pub(super) state_indices: BTreeMap<EntityId, usize>,
    pub(super) propagation_edges: Vec<PropagationEdge>,
    nodes: BTreeMap<EntityId, &'a Node>,
    edges: &'a [Edge],
}

impl<'a> AnalysisGraph<'a> {
    pub(super) fn new(
        scenario: &Scenario,
        nodes: &'a [Node],
        edges: &'a [Edge],
    ) -> Result<Self, ScenarioAnalysisError> {
        let nodes_by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        validate_references(scenario, &nodes_by_id)?;
        let relevant = scenario_analysis_reachability::relevant_states(scenario, edges);
        let states = scenario_analysis_state::project(&nodes_by_id, &relevant)?;
        let state_indices = states
            .iter()
            .enumerate()
            .map(|(index, state)| (state.id, index))
            .collect::<BTreeMap<_, _>>();
        let propagation_edges = scenario_analysis_edges::propagation(edges, &state_indices)?;
        Ok(Self {
            states,
            state_indices,
            propagation_edges,
            nodes: nodes_by_id,
            edges,
        })
    }

    pub(super) fn intervention(
        &self,
        candidate: EntityId,
    ) -> Result<(&Intervention, Vec<InterventionEdge>), ScenarioAnalysisError> {
        let NodePayload::Intervention(intervention) = &self.nodes[&candidate].payload else {
            unreachable!("validated candidate")
        };
        let edges =
            scenario_analysis_edges::intervention(candidate, self.edges, &self.state_indices);
        Ok((intervention, edges))
    }

    pub(super) fn objective_reachable(&self, candidate: EntityId, objective: EntityId) -> bool {
        let starts = self
            .edges
            .iter()
            .filter(|edge| {
                edge.source == candidate
                    && matches!(edge.payload, EdgePayload::Changes(_))
                    && self.state_indices.contains_key(&edge.destination)
            })
            .map(|edge| edge.destination)
            .collect::<Vec<_>>();
        super::scenario_analysis_reachability::reaches(
            starts,
            objective,
            &self.propagation_edges,
            &self.states,
        )
    }
}

fn validate_references(
    scenario: &Scenario,
    nodes: &BTreeMap<EntityId, &Node>,
) -> Result<(), ScenarioAnalysisError> {
    for objective in &scenario.draft.objectives {
        match nodes.get(&objective.outcome_id) {
            Some(node)
                if matches!(node.payload, NodePayload::Outcome(_))
                    && node.native_state.as_ref().is_some_and(|state| {
                        state.forecast.is_some() || state.current.is_some()
                    }) => {}
            Some(node) if matches!(node.payload, NodePayload::Outcome(_)) => {
                return Err(ScenarioAnalysisError::MissingObjectiveBaseline(
                    objective.outcome_id,
                ));
            }
            _ => {
                return Err(ScenarioAnalysisError::InvalidReference(
                    objective.outcome_id,
                ));
            }
        }
    }
    for candidate in &scenario.draft.candidate_interventions {
        if !matches!(
            nodes.get(candidate).map(|node| &node.payload),
            Some(NodePayload::Intervention(_))
        ) {
            return Err(ScenarioAnalysisError::InvalidReference(*candidate));
        }
    }
    Ok(())
}
