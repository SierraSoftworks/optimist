use std::collections::BTreeMap;

use rand::Rng;
use rand_chacha::ChaCha20Rng;

use super::effect_activation::{self, SampledEffectProfile};
use super::scenario_analysis_accumulator::Accumulator;
use super::scenario_analysis_baseline;
use super::scenario_analysis_graph::{AnalysisGraph, CandidateExecutionPlan};
use super::{Scenario, ScenarioAnalysisError, StateDetail, UtilityDirection};
use crate::squiggle::Runtime;

struct SampledPropagationEdge {
    source: usize,
    destination: usize,
    source_name: String,
    effect: f64,
    delay: u64,
}

struct SampledInterventionEdge {
    destination: usize,
    intervention_name: String,
    effect: f64,
    rebound: Option<f64>,
    arrival: u64,
    profile: SampledEffectProfile,
}

pub(super) struct ScenarioDraw {
    pub(super) values: Vec<f64>,
    pub(super) trajectories: Vec<Vec<[f64; 2]>>,
    /// Every state's value at every period, when the caller asked to keep it.
    pub(super) history: Vec<Vec<f64>>,
    pub(super) clamped_state_updates: u64,
    pub(super) undefined_responses: u64,
}

pub(super) fn draw(
    graph: &AnalysisGraph<'_>,
    scenario: &Scenario,
    execution: &CandidateExecutionPlan,
    rng: &mut ChaCha20Rng,
    runtime: &mut Runtime,
    detail: StateDetail,
) -> Result<ScenarioDraw, ScenarioAnalysisError> {
    let coupled = graph.coupling.draw(rng);
    let mut baselines = graph
        .states
        .iter()
        .map(|state| state.baseline.sample(rng, &coupled))
        .collect::<Vec<_>>();
    let parameters = graph
        .states
        .iter()
        .map(|state| {
            state
                .relation
                .as_ref()
                .map(|relation| relation.sample_parameters(rng))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    scenario_analysis_baseline::settle(&graph.states, &mut baselines, &parameters, runtime)?;
    let baselines = baselines;
    if baselines.iter().any(|value| !value.is_finite()) {
        return Err(ScenarioAnalysisError::NonFiniteResult);
    }
    let blocked = execution.blockers.iter().any(|requirement| {
        requirement.hard
            && requirement.satisfaction_threshold.is_none_or(|threshold| {
                graph
                    .state_indices
                    .get(&requirement.prerequisite)
                    .is_none_or(|index| baselines[*index] < threshold)
            })
    });
    let mut succeeds = !blocked;
    let mut completion = 0_u64;
    let mut interventions = Vec::new();
    if !blocked {
        for step in &execution.steps {
            completion = completion.saturating_add(
                step.duration
                    .as_ref()
                    .map(|estimate| effect_activation::periods(estimate.sample(rng, &coupled)))
                    .transpose()?
                    .unwrap_or(0),
            );
            let step_succeeds = rng.r#gen::<f64>()
                < step
                    .probability_of_success
                    .as_ref()
                    .map_or(1.0, |estimate| estimate.sample(rng, &coupled));
            if !step_succeeds {
                succeeds = false;
                break;
            }
            for edge in &step.edges {
                interventions.push(SampledInterventionEdge {
                    destination: edge.destination,
                    intervention_name: edge.intervention_name.clone(),
                    effect: edge.effect.sample(rng, &coupled),
                    arrival: completion
                        .saturating_add(
                            edge.lag
                                .as_ref()
                                .map(|lag| effect_activation::periods(lag.sample(rng, &coupled)))
                                .transpose()?
                                .unwrap_or(0),
                        )
                        .saturating_add(1),
                    profile: effect_activation::sample(&edge.profile, rng)?,
                    rebound: edge.rebound.as_ref().map(|rebound| rebound.sample(rng)),
                });
            }
        }
    }
    let causal = graph
        .propagation_edges
        .iter()
        .map(|edge| {
            Ok(SampledPropagationEdge {
                source: edge.source,
                destination: edge.destination,
                source_name: edge.source_name.clone(),
                effect: edge.effect.sample(rng, &coupled),
                delay: edge
                    .lag
                    .as_ref()
                    .map(|lag| effect_activation::periods(lag.sample(rng, &coupled)))
                    .transpose()?
                    .unwrap_or(0)
                    .saturating_add(1),
            })
        })
        .collect::<Result<Vec<_>, ScenarioAnalysisError>>()?;
    let mut history = vec![baselines.clone()];
    let mut clamped_state_updates = 0_u64;
    let mut undefined_responses = 0_u64;
    for period in 1..=scenario.draft.planning_horizon {
        let mut accumulator = Accumulator::new(&graph.states);
        for edge in &interventions {
            if period >= edge.arrival {
                let elapsed = period - edge.arrival;
                let state = &graph.states[edge.destination];
                accumulator.multiplier(
                    state,
                    edge.destination,
                    edge.effect,
                    edge.profile.activation(elapsed),
                );
                if let Some(rebound) = edge.rebound {
                    accumulator.multiplier(
                        state,
                        edge.destination,
                        rebound,
                        edge.profile.rebound(elapsed),
                    );
                }
            }
        }
        for edge in &causal {
            if period >= edge.delay {
                let source = history[(period - edge.delay) as usize][edge.source];
                accumulator.elasticity(
                    &graph.states[edge.destination],
                    edge.destination,
                    edge.effect,
                    source,
                    baselines[edge.source],
                );
            }
        }
        undefined_responses = undefined_responses.saturating_add(accumulator.undefined);
        let mut current = accumulator.resolve(&graph.states, &baselines);
        relate(
            graph,
            &mut current,
            &baselines,
            &history,
            &causal,
            &interventions,
            &parameters,
            period,
            runtime,
        )?;
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
    let mut values = scenario
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
        .collect::<Vec<_>>();
    values.push(completion as f64);
    values.push(if succeeds { 1.0 } else { 0.0 });
    let trajectories = scenario
        .draft
        .objectives
        .iter()
        .map(|objective| {
            let index = graph.state_indices[&objective.outcome_id];
            let baseline = baselines[index];
            history
                .iter()
                .map(|states| {
                    let state = states[index];
                    let improvement = match objective.direction {
                        UtilityDirection::Maximize => state - baseline,
                        UtilityDirection::Minimize => baseline - state,
                    };
                    [state, improvement]
                })
                .collect()
        })
        .collect();
    Ok(ScenarioDraw {
        values,
        trajectories,
        history: if detail.is_included() {
            history
        } else {
            Vec::new()
        },
        clamped_state_updates,
        undefined_responses,
    })
}

/// Overwrites every state that owns a node equation with the equation's result.
///
/// A relation replaces proportional composition rather than adding to it, so the
/// accumulator's value for that state is discarded. Parents are read from
/// history at their relationship's lag, which the mandatory one-period transport
/// delay guarantees is already complete, so relations within a period never
/// depend on each other's order.
#[allow(clippy::too_many_arguments)]
fn relate(
    graph: &AnalysisGraph<'_>,
    current: &mut [f64],
    baselines: &[f64],
    history: &[Vec<f64>],
    causal: &[SampledPropagationEdge],
    interventions: &[SampledInterventionEdge],
    parameters: &[BTreeMap<String, f64>],
    period: u64,
    runtime: &mut Runtime,
) -> Result<(), ScenarioAnalysisError> {
    for (index, state) in graph.states.iter().enumerate() {
        let Some(relation) = state.relation.as_ref() else {
            continue;
        };
        let mut bindings = relation.bindings(baselines[index], baselines, &parameters[index]);
        for edge in causal.iter().filter(|edge| edge.destination == index) {
            if period >= edge.delay {
                let value = history[(period - edge.delay) as usize][edge.source];
                relation.set_parent(&mut bindings, &edge.source_name, value);
            }
        }
        for edge in interventions
            .iter()
            .filter(|edge| edge.destination == index)
        {
            if period >= edge.arrival {
                let activation = edge.profile.activation(period - edge.arrival);
                relation.set_activation(&mut bindings, &edge.intervention_name, activation);
            }
        }
        current[index] = relation.evaluate(runtime, &bindings)?;
    }
    Ok(())
}
