use std::collections::BTreeMap;

use super::{
    Distribution, Edge, EdgePayload, EffectProfile, EntityId, NodeKind, ScenarioAnalysisError,
};

pub(super) struct PropagationEdge {
    pub(super) source: usize,
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) lag: Option<Distribution>,
}

pub(super) struct InterventionEdge {
    pub(super) destination: usize,
    pub(super) effect: Distribution,
    pub(super) rebound: Option<Distribution>,
    pub(super) profile: EffectProfile,
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
                Some((edge, &effect.response.distribution, effect.lag.as_ref()))
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
                    effect: effect.response.distribution.clone(),
                    rebound: effect
                        .transience
                        .as_ref()
                        .and_then(|transience| transience.rebound.as_ref())
                        .map(|estimate| estimate.distribution.clone()),
                    profile: effect
                        .transience
                        .as_ref()
                        .map(|transience| transience.profile.clone())
                        .unwrap_or_default(),
                    lag: effect.lag.as_ref().map(|lag| lag.distribution.clone()),
                })
            }
            _ => None,
        })
        .collect()
}
