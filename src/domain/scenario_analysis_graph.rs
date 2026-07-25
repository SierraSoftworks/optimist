use std::collections::BTreeMap;

use super::{
    Edge, EdgePayload, EntityId, EstimateOwner, Node, NodePayload, ProjectDependenceModel,
    ProjectId, Scenario, ScenarioAnalysisError, intervention_execution,
    scenario_analysis_coupling::{CoupledPrimitive, Coupling},
    scenario_analysis_edges::{self, InterventionEdge, PropagationEdge},
    scenario_analysis_reachability,
    scenario_analysis_state::{self, StateNode},
};

pub(super) struct PlannedIntervention {
    pub(super) id: EntityId,
    pub(super) duration: Option<CoupledPrimitive>,
    pub(super) probability_of_success: Option<CoupledPrimitive>,
    pub(super) edges: Vec<InterventionEdge>,
}

pub(super) struct CandidateExecutionPlan {
    pub(super) steps: Vec<PlannedIntervention>,
    pub(super) blockers: Vec<intervention_execution::ExecutionRequirement>,
    pub(super) synergies: Vec<EntityId>,
    pub(super) conflicts: Vec<EntityId>,
}

pub(super) struct AnalysisGraph<'a> {
    pub(super) states: Vec<StateNode>,
    pub(super) state_indices: BTreeMap<EntityId, usize>,
    pub(super) propagation_edges: Vec<PropagationEdge>,
    pub(super) coupling: Coupling,
    nodes: BTreeMap<EntityId, &'a Node>,
    edges: &'a [Edge],
}

impl<'a> AnalysisGraph<'a> {
    pub(super) fn new(
        project: &ProjectId,
        scenario: &Scenario,
        nodes: &'a [Node],
        edges: &'a [Edge],
        dependence: Option<&ProjectDependenceModel>,
    ) -> Result<Self, ScenarioAnalysisError> {
        let nodes_by_id: BTreeMap<_, _> = nodes.iter().map(|node| (node.id, node)).collect();
        validate_references(scenario, &nodes_by_id)?;
        let coupling = Coupling::new(project, dependence);
        let relevant =
            scenario_analysis_reachability::relevant_states(scenario, &nodes_by_id, edges);
        let states = scenario_analysis_state::project(&nodes_by_id, &relevant, edges, &coupling)?;
        let state_indices = states
            .iter()
            .enumerate()
            .map(|(index, state)| (state.id, index))
            .collect::<BTreeMap<_, _>>();
        let propagation_edges =
            scenario_analysis_edges::propagation(&nodes_by_id, edges, &state_indices, &coupling)?;
        Ok(Self {
            states,
            state_indices,
            propagation_edges,
            coupling,
            nodes: nodes_by_id,
            edges,
        })
    }

    pub(super) fn intervention_plan(
        &self,
        candidate: EntityId,
    ) -> Result<CandidateExecutionPlan, ScenarioAnalysisError> {
        let plan = intervention_execution::plan(candidate, &self.nodes, self.edges)
            .map_err(ScenarioAnalysisError::InterventionDependencyCycle)?;
        Ok(CandidateExecutionPlan {
            steps: plan
                .steps
                .into_iter()
                .map(|(id, intervention)| {
                    let owner = EstimateOwner::Node(id);
                    PlannedIntervention {
                        id,
                        duration: intervention.duration.as_ref().map(|estimate| {
                            self.coupling
                                .primitive(&owner, estimate.id, &estimate.distribution)
                        }),
                        probability_of_success: intervention.probability_of_success.as_ref().map(
                            |estimate| {
                                self.coupling
                                    .primitive(&owner, estimate.id, &estimate.distribution)
                            },
                        ),
                        edges: scenario_analysis_edges::intervention(
                            id,
                            self.nodes.get(&id).map_or("", |node| node.name.as_str()),
                            self.edges,
                            &self.state_indices,
                            &self.coupling,
                        ),
                    }
                })
                .collect(),
            blockers: plan.blockers,
            synergies: plan.synergies,
            conflicts: plan.conflicts,
        })
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
