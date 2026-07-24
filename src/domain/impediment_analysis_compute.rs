use std::collections::BTreeMap;

use super::{
    AnalysisError, AnalysisRevisionKey, Edge, EntityId, ImpedimentAnalysis, ImpedimentCandidate,
    InterventionExecutionStep, InterventionRequirement, Node, NodePayload, intervention_execution,
};

impl ImpedimentAnalysis {
    /// Computes intervention execution readiness from requirements, synergies, and conflicts.
    pub fn compute(
        revision: AnalysisRevisionKey,
        nodes: &[Node],
        edges: &[Edge],
    ) -> Result<Self, AnalysisError> {
        let nodes_by_id = nodes
            .iter()
            .map(|node| (node.id, node))
            .collect::<BTreeMap<_, _>>();
        let mut candidates = nodes
            .iter()
            .filter(|node| matches!(node.payload, NodePayload::Intervention(_)))
            .map(|node| readiness(node.id, &nodes_by_id, edges))
            .collect::<Result<Vec<_>, _>>()?;
        candidates.sort_by(|left, right| {
            hard_blocker_count(left)
                .cmp(&hard_blocker_count(right))
                .then_with(|| {
                    right
                        .expected_success_probability
                        .total_cmp(&left.expected_success_probability)
                })
                .then_with(|| left.expected_duration.total_cmp(&right.expected_duration))
                .then(left.intervention.cmp(&right.intervention))
        });
        Ok(Self {
            revision,
            candidates,
        })
    }
}

fn hard_blocker_count(candidate: &ImpedimentCandidate) -> usize {
    candidate
        .blocking_requirements
        .iter()
        .filter(|requirement| requirement.hard)
        .count()
}

fn readiness(
    candidate: EntityId,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
) -> Result<ImpedimentCandidate, AnalysisError> {
    let plan = intervention_execution::plan(candidate, nodes, edges)
        .map_err(AnalysisError::InterventionDependencyCycle)?;
    let execution_steps = plan
        .steps
        .into_iter()
        .map(|(intervention, value)| InterventionExecutionStep {
            intervention,
            duration: value
                .duration
                .as_ref()
                .map(|estimate| estimate.distribution.clone()),
            probability_of_success: value
                .probability_of_success
                .as_ref()
                .map(|estimate| estimate.distribution.clone()),
        })
        .collect::<Vec<_>>();
    let expected_duration = execution_steps
        .iter()
        .filter_map(|step| step.duration.as_ref())
        .map(|distribution| distribution.mean())
        .sum();
    let expected_success_probability = execution_steps
        .iter()
        .filter_map(|step| step.probability_of_success.as_ref())
        .map(|distribution| distribution.mean())
        .product();
    Ok(ImpedimentCandidate {
        intervention: candidate,
        execution_steps,
        blocking_requirements: plan
            .blockers
            .into_iter()
            .map(|requirement| InterventionRequirement {
                dependent: requirement.dependent,
                prerequisite: requirement.prerequisite,
                hard: requirement.hard,
                satisfaction_threshold: requirement.satisfaction_threshold,
            })
            .collect(),
        synergies: plan.synergies,
        conflicts: plan.conflicts,
        expected_duration,
        expected_success_probability,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Distribution, EdgePayload, Estimate, EstimateId, Intervention, NodeKind, ProjectId,
        Requirement,
    };

    fn revision() -> AnalysisRevisionKey {
        AnalysisRevisionKey {
            project: ProjectId::new("readiness").unwrap(),
            graph_revision: 4,
            scenario: None,
            dependence_revision: None,
        }
    }

    fn intervention(id: u64, duration: f64, probability: f64) -> Node {
        Node::new(
            EntityId::new(id),
            format!("intervention-{id}"),
            format!("Intervention {id}"),
            NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: Some(
                    Estimate::new(EstimateId::new(0), Distribution::point(duration).unwrap())
                        .unwrap(),
                ),
                probability_of_success: Some(
                    Estimate::new(
                        EstimateId::new(1),
                        Distribution::point(probability).unwrap(),
                    )
                    .unwrap(),
                ),
                acceptance_criteria: vec![],
            }),
        )
        .unwrap()
    }

    fn requires(source: u64, destination: u64) -> Edge {
        Edge::new(
            EntityId::new(source),
            NodeKind::Intervention,
            EntityId::new(destination),
            NodeKind::Intervention,
            EdgePayload::Requires(Requirement {
                hard: true,
                satisfaction_threshold: None,
            }),
        )
        .unwrap()
    }

    #[test]
    fn orders_dependencies_and_compounds_duration_and_success() {
        let result = ImpedimentAnalysis::compute(
            revision(),
            &[intervention(0, 3.0, 0.8), intervention(1, 2.0, 0.5)],
            &[requires(0, 1)],
        )
        .unwrap();
        let candidate = result
            .candidates
            .iter()
            .find(|value| value.intervention == EntityId::new(0))
            .unwrap();
        assert_eq!(
            candidate
                .execution_steps
                .iter()
                .map(|step| step.intervention)
                .collect::<Vec<_>>(),
            vec![EntityId::new(1), EntityId::new(0)]
        );
        assert_eq!(candidate.expected_duration, 5.0);
        assert!((candidate.expected_success_probability - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn rejects_intervention_dependency_cycles() {
        let result = ImpedimentAnalysis::compute(
            revision(),
            &[intervention(0, 1.0, 1.0), intervention(1, 1.0, 1.0)],
            &[requires(0, 1), requires(1, 0)],
        );
        assert!(matches!(
            result,
            Err(AnalysisError::InterventionDependencyCycle(_))
        ));
    }
}
