use std::collections::{BTreeMap, BTreeSet};

use super::{Edge, EdgePayload, EntityId, Intervention, Node, NodePayload};

pub(super) struct ExecutionPlan<'a> {
    pub(super) steps: Vec<(EntityId, &'a Intervention)>,
    pub(super) blockers: Vec<ExecutionRequirement>,
    pub(super) synergies: Vec<EntityId>,
    pub(super) conflicts: Vec<EntityId>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExecutionRequirement {
    pub(super) dependent: EntityId,
    pub(super) prerequisite: EntityId,
    pub(super) hard: bool,
    pub(super) satisfaction_threshold: Option<f64>,
}

pub(super) fn plan<'a>(
    candidate: EntityId,
    nodes: &BTreeMap<EntityId, &'a Node>,
    edges: &[Edge],
) -> Result<ExecutionPlan<'a>, EntityId> {
    let mut steps = Vec::new();
    let mut blockers = Vec::new();
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    visit(
        candidate,
        nodes,
        edges,
        &mut visiting,
        &mut visited,
        &mut steps,
        &mut blockers,
    )?;
    let step_ids = steps.iter().map(|(id, _)| *id).collect::<BTreeSet<_>>();
    let related = |predicate: fn(&EdgePayload) -> bool| {
        edges
            .iter()
            .filter(|edge| predicate(&edge.payload))
            .flat_map(|edge| {
                let connected =
                    step_ids.contains(&edge.source) || step_ids.contains(&edge.destination);
                connected
                    .then_some([edge.source, edge.destination])
                    .into_iter()
                    .flatten()
            })
            .filter(|id| *id != candidate)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    Ok(ExecutionPlan {
        steps,
        blockers,
        synergies: related(|payload| matches!(payload, EdgePayload::SynergizesWith)),
        conflicts: related(|payload| matches!(payload, EdgePayload::ConflictsWith)),
    })
}

#[allow(clippy::too_many_arguments)]
fn visit<'a>(
    intervention: EntityId,
    nodes: &BTreeMap<EntityId, &'a Node>,
    edges: &[Edge],
    visiting: &mut BTreeSet<EntityId>,
    visited: &mut BTreeSet<EntityId>,
    steps: &mut Vec<(EntityId, &'a Intervention)>,
    blockers: &mut Vec<ExecutionRequirement>,
) -> Result<(), EntityId> {
    if visited.contains(&intervention) {
        return Ok(());
    }
    if !visiting.insert(intervention) {
        return Err(intervention);
    }
    let mut requirements = edges
        .iter()
        .filter(|edge| edge.source == intervention)
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Requires(requirement) => Some((edge.destination, requirement)),
            _ => None,
        })
        .collect::<Vec<_>>();
    requirements.sort_by_key(|(id, _)| *id);
    for (prerequisite, requirement) in requirements {
        match nodes.get(&prerequisite).map(|node| &node.payload) {
            Some(NodePayload::Intervention(_)) => visit(
                prerequisite,
                nodes,
                edges,
                visiting,
                visited,
                steps,
                blockers,
            )?,
            Some(NodePayload::Factor(_)) => blockers.push(ExecutionRequirement {
                dependent: intervention,
                prerequisite,
                hard: requirement.hard,
                satisfaction_threshold: requirement.satisfaction_threshold,
            }),
            _ => {}
        }
    }
    visiting.remove(&intervention);
    visited.insert(intervention);
    let NodePayload::Intervention(value) = &nodes[&intervention].payload else {
        unreachable!("execution plans start from validated interventions")
    };
    steps.push((intervention, value));
    Ok(())
}
