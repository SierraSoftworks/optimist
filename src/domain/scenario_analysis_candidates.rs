use std::{num::NonZeroUsize, thread::available_parallelism};

use super::{
    InterventionProjection, Scenario, ScenarioAnalysisError,
    scenario_analysis_graph::AnalysisGraph, scenario_analysis_sampling,
};

/// Projects every candidate in the scenario, sharing the work across cores.
///
/// Each candidate seeds its own ChaCha20 stream from the scenario's seed and
/// reads the graph without mutating it, so the candidates are already
/// independent of one another and evaluating them together changes no draw. The
/// only per-candidate mutable state is the relation runtime, which each worker
/// builds for itself.
///
/// Work is split into contiguous chunks rather than handed out one candidate at
/// a time, so the projections come back in the order the scenario lists its
/// candidates without needing to be sorted.
pub(super) fn project_candidates(
    graph: &AnalysisGraph<'_>,
    scenario: &Scenario,
) -> Result<Vec<InterventionProjection>, ScenarioAnalysisError> {
    let candidates = scenario.draft.candidate_interventions.as_slice();
    let workers = available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(candidates.len());
    if workers < 2 {
        return candidates
            .iter()
            .map(|candidate| {
                scenario_analysis_sampling::project_candidate(graph, *candidate, scenario)
            })
            .collect();
    }
    std::thread::scope(|scope| {
        let handles = candidates
            .chunks(candidates.len().div_ceil(workers))
            .map(|chunk| scope.spawn(move || project_chunk(graph, chunk, scenario)))
            .collect::<Vec<_>>();
        let mut projections = Vec::with_capacity(candidates.len());
        for handle in handles {
            let chunk = handle
                .join()
                .map_err(|_| ScenarioAnalysisError::Panicked)??;
            projections.extend(chunk);
        }
        Ok(projections)
    })
}

fn project_chunk(
    graph: &AnalysisGraph<'_>,
    candidates: &[super::EntityId],
    scenario: &Scenario,
) -> Result<Vec<InterventionProjection>, ScenarioAnalysisError> {
    candidates
        .iter()
        .map(|candidate| scenario_analysis_sampling::project_candidate(graph, *candidate, scenario))
        .collect()
}
