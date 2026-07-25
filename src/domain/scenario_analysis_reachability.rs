use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    Edge, EdgePayload, EntityId, Node, NodeKind, Scenario,
    scenario_analysis_edges::PropagationEdge, scenario_analysis_state::StateNode,
    state_relation_schema,
};

pub(super) fn relevant_states(
    scenario: &Scenario,
    nodes: &BTreeMap<EntityId, &Node>,
    edges: &[Edge],
) -> BTreeSet<EntityId> {
    let causal = edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.payload,
                EdgePayload::Contributes(_) | EdgePayload::Blocks(_)
            ) && edge.destination_kind != NodeKind::Intervention
        })
        .collect::<Vec<_>>();
    let forward = adjacency(
        causal
            .iter()
            .copied()
            .map(|edge| (edge.source, edge.destination)),
    );
    let reverse = adjacency(
        causal
            .iter()
            .copied()
            .map(|edge| (edge.destination, edge.source)),
    );
    let starts = edges
        .iter()
        .filter(|edge| {
            matches!(edge.payload, EdgePayload::Changes(_))
                && scenario
                    .draft
                    .candidate_interventions
                    .contains(&edge.source)
        })
        .map(|edge| edge.destination)
        .collect::<BTreeSet<_>>();
    let objectives = scenario
        .draft
        .objectives
        .iter()
        .map(|objective| objective.outcome_id)
        .collect::<BTreeSet<_>>();
    let reachable_from_candidates = closure(starts.iter().copied(), &forward);
    let can_reach_objectives = closure(objectives.iter().copied(), &reverse);
    let moved = reachable_from_candidates
        .intersection(&can_reach_objectives)
        .copied()
        .chain(objectives)
        .collect();
    with_relation_parents(moved, nodes, &reverse)
}

/// Adds every parent a projected node equation reads, transitively.
///
/// Proportional composition can ignore a parent nothing moves, because an
/// unchanged parent contributes a ratio of one. An equation cannot: it computes
/// the whole value from all of its inputs, so a parent left out would change
/// what the equation means rather than merely contributing nothing. Those
/// parents are projected and hold their baselines.
fn with_relation_parents(
    mut relevant: BTreeSet<EntityId>,
    nodes: &BTreeMap<EntityId, &Node>,
    reverse: &BTreeMap<EntityId, Vec<EntityId>>,
) -> BTreeSet<EntityId> {
    let mut pending: VecDeque<_> = relevant.iter().copied().collect();
    while let Some(id) = pending.pop_front() {
        if nodes
            .get(&id)
            .is_none_or(|node| state_relation_schema::relation_of(node).is_none())
        {
            continue;
        }
        for parent in reverse.get(&id).into_iter().flatten() {
            if relevant.insert(*parent) {
                pending.push_back(*parent);
            }
        }
    }
    relevant
}

pub(super) fn reaches(
    starts: Vec<EntityId>,
    objective: EntityId,
    edges: &[PropagationEdge],
    states: &[StateNode],
) -> bool {
    let ids = states.iter().map(|state| state.id).collect::<Vec<_>>();
    let adjacency = adjacency(
        edges
            .iter()
            .map(|edge| (ids[edge.source], ids[edge.destination])),
    );
    closure(starts.into_iter(), &adjacency).contains(&objective)
}

fn adjacency(
    edges: impl Iterator<Item = (EntityId, EntityId)>,
) -> BTreeMap<EntityId, Vec<EntityId>> {
    let mut result = BTreeMap::<_, Vec<_>>::new();
    for (source, destination) in edges {
        result.entry(source).or_default().push(destination);
    }
    result
}

fn closure(
    seeds: impl Iterator<Item = EntityId>,
    adjacency: &BTreeMap<EntityId, Vec<EntityId>>,
) -> BTreeSet<EntityId> {
    let mut visited = BTreeSet::new();
    let mut queue = seeds.collect::<VecDeque<_>>();
    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) {
            continue;
        }
        queue.extend(adjacency.get(&node).into_iter().flatten().copied());
    }
    visited
}
