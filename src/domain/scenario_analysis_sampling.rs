use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use super::{
    ConvergenceStatus, EntityId, InterventionProjection, InvalidSampleCounts,
    MonteCarloDiagnostics, MonteCarloEstimate, ObjectiveProjection, ObjectiveTrajectoryPoint,
    RelationProgram, Scenario, ScenarioAnalysisError, StateDetail, StateTrajectory,
    online_moments::OnlineJointMoments, scenario_analysis_draw,
    scenario_analysis_graph::AnalysisGraph,
};

pub(super) fn project_candidate(
    graph: &AnalysisGraph<'_>,
    candidate: EntityId,
    scenario: &Scenario,
    detail: StateDetail,
) -> Result<InterventionProjection, ScenarioAnalysisError> {
    let execution = graph.intervention_plan(candidate)?;
    let config = scenario.draft.monte_carlo;
    let objective_dimensions = scenario.draft.objectives.len() * 3;
    let dimensions = objective_dimensions + 2;
    let mut moments = OnlineJointMoments::new(dimensions);
    let mut trajectory_moments = scenario
        .draft
        .objectives
        .iter()
        .map(|_| {
            (0..=scenario.draft.planning_horizon)
                .map(|_| OnlineJointMoments::new(2))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed());
    // Building the standard environment dominates a relation evaluation, so one
    // runtime is reused across every draw and period rather than rebuilt.
    let mut runtime = RelationProgram::runtime(config.seed())
        .map_err(|error| ScenarioAnalysisError::Relation(error.to_string()))?;
    let mut attempted = 0;
    let mut invalid = InvalidSampleCounts::default();
    let mut clamped_state_updates = 0_u64;
    let mut undefined_responses = 0_u64;
    // One accumulator per state per period, so a path can be read back with the
    // same online moments the objectives use.
    let mut state_moments = detail.is_included().then(|| {
        (0..graph.states.len())
            .map(|_| {
                (0..=scenario.draft.planning_horizon)
                    .map(|_| OnlineJointMoments::new(1))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    while attempted < config.maximum_samples() {
        attempted += 1;
        match scenario_analysis_draw::draw(
            graph,
            scenario,
            &execution,
            &mut rng,
            &mut runtime,
            detail,
        ) {
            Ok(draw) => {
                moments.push(&draw.values);
                for (objective, trajectory) in trajectory_moments.iter_mut().zip(draw.trajectories)
                {
                    for (period, values) in objective.iter_mut().zip(trajectory) {
                        period.push(&values);
                    }
                }
                if let Some(states) = state_moments.as_mut() {
                    for (period, values) in draw.history.iter().enumerate() {
                        for (index, value) in values.iter().enumerate() {
                            if let Some(slot) = states[index].get_mut(period) {
                                slot.push(&[*value]);
                            }
                        }
                    }
                }
                clamped_state_updates =
                    clamped_state_updates.saturating_add(draw.clamped_state_updates);
                undefined_responses = undefined_responses.saturating_add(draw.undefined_responses);
            }
            Err(ScenarioAnalysisError::NonFinitePrimitive) => invalid.non_finite_primitive += 1,
            Err(ScenarioAnalysisError::NonFiniteResult) => invalid.non_finite_result += 1,
            Err(error) => return Err(error),
        }
        if config.converged(&moments, dimensions) {
            break;
        }
    }
    let objectives = scenario
        .draft
        .objectives
        .iter()
        .enumerate()
        .map(|(index, objective)| {
            let periods_to_effect = graph.periods_to_effect(candidate, objective.outcome_id);
            ObjectiveProjection {
                outcome: objective.outcome_id,
                direction: objective.direction,
                importance: objective.importance,
                reachable: periods_to_effect.is_some(),
                periods_to_effect,
                baseline: estimate(&moments, index * 3),
                final_state: estimate(&moments, index * 3 + 1),
                improvement: estimate(&moments, index * 3 + 2),
                trajectory: trajectory_moments[index]
                    .iter()
                    .enumerate()
                    .map(|(period, moments)| ObjectiveTrajectoryPoint {
                        period: period as u64,
                        state: estimate(moments, 0),
                        improvement: estimate(moments, 1),
                    })
                    .collect(),
            }
        })
        .collect();
    let improvement_covariance = (0..scenario.draft.objectives.len())
        .map(|row| {
            (0..scenario.draft.objectives.len())
                .map(|column| moments.covariance(row * 3 + 2, column * 3 + 2))
                .collect()
        })
        .collect();
    let status = if config.converged(&moments, dimensions) {
        ConvergenceStatus::Converged
    } else if moments.count() < config.minimum_samples() {
        ConvergenceStatus::InsufficientValidSamples
    } else {
        ConvergenceStatus::MaximumSamplesReached
    };
    Ok(InterventionProjection {
        intervention: candidate,
        prerequisites: execution
            .steps
            .iter()
            .map(|step| step.id)
            .filter(|id| *id != candidate)
            .collect(),
        blocking_requirements: execution
            .blockers
            .iter()
            .map(|requirement| super::InterventionRequirement {
                dependent: requirement.dependent,
                prerequisite: requirement.prerequisite,
                hard: requirement.hard,
                satisfaction_threshold: requirement.satisfaction_threshold,
            })
            .collect(),
        synergies: execution.synergies.clone(),
        conflicts: execution.conflicts.clone(),
        execution_duration: estimate(&moments, objective_dimensions),
        execution_success: estimate(&moments, objective_dimensions + 1),
        objectives,
        states: state_moments.map_or_else(Vec::new, |states| {
            states
                .into_iter()
                .zip(&graph.states)
                .map(|(periods, state)| StateTrajectory {
                    state: state.id,
                    points: periods.iter().map(|moments| estimate(moments, 0)).collect(),
                })
                .collect()
        }),
        improvement_covariance,
        clamped_state_updates,
        undefined_responses,
        diagnostics: MonteCarloDiagnostics {
            seed: config.seed(),
            attempted_samples: attempted,
            valid_samples: moments.count(),
            invalid_samples: invalid,
            criterion: config,
            status,
        },
    })
}

fn estimate(moments: &OnlineJointMoments, index: usize) -> MonteCarloEstimate {
    MonteCarloEstimate {
        mean: moments.mean(index),
        variance: moments.variance(index),
        mean_standard_error: moments.mean_standard_error(index),
        variance_standard_error: moments.variance_standard_error(index),
    }
}
