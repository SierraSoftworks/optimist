use std::collections::BTreeMap;

use super::{ScenarioAnalysisError, scenario_analysis_state::StateNode};
use crate::squiggle::Runtime;

/// Replaces every equation-backed baseline with the equation evaluated at rest.
///
/// A node equation defines its state, so the state's value before any
/// intervention is the equation applied to the parents' own baselines with every
/// activation at zero. Sampling the authored estimate instead would draw the
/// same quantity twice, once through the equation and once directly, and the
/// objective would then compare a projection against an unrelated draw. The
/// resulting "improvement" would be dominated by the gap between two independent
/// samples rather than by the intervention.
///
/// Equations may read other equations, so states are settled in dependency
/// order. A state whose parents cannot all be settled first sits on a cycle,
/// where the equation has no closed-form rest point; that state keeps its
/// authored estimate, which is the author's own statement of where the loop
/// settles.
pub(super) fn settle(
    states: &[StateNode],
    baselines: &mut [f64],
    parameters: &[BTreeMap<String, f64>],
    runtime: &mut Runtime,
) -> Result<(), ScenarioAnalysisError> {
    let mut settled = states
        .iter()
        .map(|state| state.relation.is_none())
        .collect::<Vec<_>>();
    while let Some(index) = next_settleable(states, &settled) {
        let relation = states[index]
            .relation
            .as_ref()
            .expect("only equation-backed states remain unsettled");
        let bindings = relation.bindings(baselines[index], baselines, &parameters[index]);
        baselines[index] = relation.evaluate(runtime, &bindings)?;
        settled[index] = true;
    }
    Ok(())
}

fn next_settleable(states: &[StateNode], settled: &[bool]) -> Option<usize> {
    states.iter().enumerate().position(|(index, state)| {
        !settled[index]
            && state
                .relation
                .as_ref()
                .is_some_and(|relation| relation.parent_indices().all(|parent| settled[parent]))
    })
}
