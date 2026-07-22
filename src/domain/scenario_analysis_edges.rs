use std::collections::BTreeMap;

use super::{Distribution, Edge, EdgePayload, EntityId, NodeKind, ScenarioAnalysisError};

pub(super) struct PropagationEdge {
    pub(super) source: usize,
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) source_change: f64,
    pub(super) lag: Option<Distribution>,
}

pub(super) struct InterventionEdge {
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) source_change: f64,
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
            EdgePayload::Contributes(effect) => {
                if let Some(value) = effect.normalized_effect() {
                    Some((edge, &value.distribution, 1.0, effect.lag.as_ref()))
                } else {
                    effect.linear_response().map(|response| {
                        (
                            edge,
                            &response.destination_change.distribution,
                            response.source_change,
                            effect.lag.as_ref(),
                        )
                    })
                }
            }
            EdgePayload::Blocks(effect) if edge.destination_kind != NodeKind::Intervention => {
                Some((edge, &effect.degree.distribution, 1.0, None))
            }
            _ => None,
        })
        .map(|(edge, effect, source_change, lag)| {
            Ok(PropagationEdge {
                source: indices[&edge.source],
                destination: indices[&edge.destination],
                effect: effect.clone(),
                source_change,
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
                effect
                    .normalized_effect()
                    .map(|value| (&value.distribution, 1.0))
                    .or_else(|| {
                        effect.linear_response().map(|response| {
                            (
                                &response.destination_change.distribution,
                                response.source_change,
                            )
                        })
                    })
                    .map(|(distribution, source_change)| InterventionEdge {
                        destination: indices[&edge.destination],
                        effect: distribution.clone(),
                        source_change,
                        lag: effect.lag.as_ref().map(|lag| lag.distribution.clone()),
                    })
            }
            _ => None,
        })
        .collect()
}
