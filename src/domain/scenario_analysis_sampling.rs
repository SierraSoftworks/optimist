use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use super::{
    ConvergenceStatus, EntityId, InterventionProjection, InvalidSampleCounts,
    MonteCarloDiagnostics, MonteCarloEstimate, ObjectiveProjection, Scenario,
    ScenarioAnalysisError, online_moments::OnlineJointMoments, scenario_analysis_draw,
    scenario_analysis_graph::AnalysisGraph,
};

pub(super) fn project_candidate(
    graph: &AnalysisGraph<'_>,
    candidate: EntityId,
    scenario: &Scenario,
) -> Result<InterventionProjection, ScenarioAnalysisError> {
    let (intervention, edges) = graph.intervention(candidate)?;
    let config = scenario.draft.monte_carlo;
    let dimensions = scenario.draft.objectives.len() * 3;
    let mut moments = OnlineJointMoments::new(dimensions);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed());
    let mut attempted = 0;
    let mut invalid = InvalidSampleCounts::default();
    let mut clamped_state_updates = 0_u64;
    while attempted < config.maximum_samples() {
        attempted += 1;
        match scenario_analysis_draw::draw(graph, scenario, intervention, &edges, &mut rng) {
            Ok(draw) => {
                moments.push(&draw.values);
                clamped_state_updates =
                    clamped_state_updates.saturating_add(draw.clamped_state_updates);
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
        .map(|(index, objective)| ObjectiveProjection {
            outcome: objective.outcome_id,
            direction: objective.direction,
            importance: objective.importance,
            reachable: graph.objective_reachable(candidate, objective.outcome_id),
            baseline: estimate(&moments, index * 3),
            final_state: estimate(&moments, index * 3 + 1),
            improvement: estimate(&moments, index * 3 + 2),
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
        objectives,
        improvement_covariance,
        clamped_state_updates,
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
