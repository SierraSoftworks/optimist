use std::collections::BTreeMap;

use super::{Distribution, Edge, EdgePayload, EntityId, NodeKind, ScenarioAnalysisError};

pub(super) struct PropagationEdge {
    pub(super) source: usize,
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) lag: Option<Distribution>,
}

pub(super) struct InterventionEdge {
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) lag: Option<Distribution>,
}

pub(super) fn propagation(
    edges: &[Edge],
    indices: &BTreeMap<EntityId, usize>,
) -> Result<Vec<PropagationEdge>, ScenarioAnalysisError> {
    edges
        .iter()
        .filter(|edge| {
            indices.contains_key(&edge.source) && indices.contains_key(&edge.destination)
        })
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Contributes(effect) if edge.destination_kind != NodeKind::Metric => {
                Some((edge, &effect.effect.distribution, effect.lag.as_ref()))
            }
            EdgePayload::Blocks(effect) if edge.destination_kind != NodeKind::Intervention => {
                Some((edge, &effect.degree.distribution, None))
            }
            _ => None,
        })
        .map(|(edge, effect, lag)| {
            Ok(PropagationEdge {
                source: indices[&edge.source],
                destination: indices[&edge.destination],
                effect: effect.clone(),
                lag: lag.map(|estimate| estimate.distribution.clone()),
            })
        })
        .collect()
}

pub(super) fn intervention(
    candidate: EntityId,
    edges: &[Edge],
    indices: &BTreeMap<EntityId, usize>,
) -> Vec<InterventionEdge> {
    edges
        .iter()
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Changes(effect)
                if edge.source == candidate && indices.contains_key(&edge.destination) =>
            {
                Some(InterventionEdge {
                    destination: indices[&edge.destination],
                    effect: effect.effect.distribution.clone(),
                    lag: effect.lag.as_ref().map(|lag| lag.distribution.clone()),
                })
            }
            _ => None,
        })
        .collect()
}
