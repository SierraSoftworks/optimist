use rand::Rng;
use rand_chacha::ChaCha20Rng;

use super::scenario_analysis_edges::InterventionEdge;
use super::scenario_analysis_graph::AnalysisGraph;
use super::{Distribution, Intervention, Scenario, ScenarioAnalysisError, UtilityDirection};

struct SampledPropagationEdge {
    source: usize,
    destination: usize,
    effect: f64,
    delay: u64,
}

struct SampledInterventionEdge {
    destination: usize,
    effect: f64,
    arrival: u64,
}

pub(super) struct ScenarioDraw {
    pub(super) values: Vec<f64>,
    pub(super) clamped_state_updates: u64,
}

pub(super) fn draw(
    graph: &AnalysisGraph<'_>,
    scenario: &Scenario,
    intervention: &Intervention,
    intervention_edges: &[InterventionEdge],
    rng: &mut ChaCha20Rng,
) -> Result<ScenarioDraw, ScenarioAnalysisError> {
    let baselines = graph
        .states
        .iter()
        .map(|state| state.baseline.sample(rng))
        .collect::<Vec<_>>();
    if baselines.iter().any(|value| !value.is_finite()) {
        return Err(ScenarioAnalysisError::NonFiniteResult);
    }
    let succeeds = rng.r#gen::<f64>()
        < intervention
            .probability_of_success
            .as_ref()
            .map_or(1.0, |estimate| estimate.distribution.sample(rng));
    let completion = intervention
        .duration
        .as_ref()
        .map(|estimate| delay(&estimate.distribution, rng))
        .transpose()?
        .unwrap_or(0);
    let interventions = intervention_edges
        .iter()
        .map(|edge| {
            Ok(SampledInterventionEdge {
                destination: edge.destination,
                effect: edge.effect.sample(rng),
                arrival: completion
                    .saturating_add(
                        edge.lag
                            .as_ref()
                            .map(|lag| delay(lag, rng))
                            .transpose()?
                            .unwrap_or(0),
                    )
                    .saturating_add(1),
            })
        })
        .collect::<Result<Vec<_>, ScenarioAnalysisError>>()?;
    let causal = graph
        .propagation_edges
        .iter()
        .map(|edge| {
            Ok(SampledPropagationEdge {
                source: edge.source,
                destination: edge.destination,
                effect: edge.effect.sample(rng) / edge.source_change,
                delay: edge
                    .lag
                    .as_ref()
                    .map(|lag| delay(lag, rng))
                    .transpose()?
                    .unwrap_or(0)
                    .saturating_add(1),
            })
        })
        .collect::<Result<Vec<_>, ScenarioAnalysisError>>()?;
    let mut history = vec![baselines.clone()];
    let mut clamped_state_updates = 0_u64;
    for period in 1..=scenario.draft.planning_horizon {
        let mut current = baselines.clone();
        if succeeds {
            for edge in &interventions {
                if period >= edge.arrival {
                    current[edge.destination] += edge.effect;
                }
            }
        }
        for edge in &causal {
            if period >= edge.delay {
                let source = history[(period - edge.delay) as usize][edge.source];
                current[edge.destination] += edge.effect * (source - baselines[edge.source]);
            }
        }
        if current.iter().any(|value| !value.is_finite()) {
            return Err(ScenarioAnalysisError::NonFiniteResult);
        }
        for (value, state) in current.iter_mut().zip(&graph.states) {
            let clamped = state.bounds.clamp(*value);
            if clamped != *value {
                clamped_state_updates = clamped_state_updates.saturating_add(1);
            }
            *value = clamped;
        }
        history.push(current);
    }
    let final_state = history.last().expect("baseline plus positive horizon");
    let values = scenario
        .draft
        .objectives
        .iter()
        .flat_map(|objective| {
            let index = graph.state_indices[&objective.outcome_id];
            let baseline = baselines[index];
            let final_value = final_state[index];
            let improvement = match objective.direction {
                UtilityDirection::Maximize => final_value - baseline,
                UtilityDirection::Minimize => baseline - final_value,
            };
            [baseline, final_value, improvement]
        })
        .collect();
    Ok(ScenarioDraw {
        values,
        clamped_state_updates,
    })
}

fn delay(distribution: &Distribution, rng: &mut ChaCha20Rng) -> Result<u64, ScenarioAnalysisError> {
    let value = distribution.sample(rng);
    if !value.is_finite() {
        return Err(ScenarioAnalysisError::NonFinitePrimitive);
    }
    Ok(value.ceil().min(u64::MAX as f64) as u64)
}
