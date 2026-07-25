use std::collections::BTreeMap;

use super::{
    Distribution, Edge, EdgePayload, EffectProfile, EntityId, EstimateId, EstimateOwner, NodeKind,
    ScenarioAnalysisError,
    scenario_analysis_coupling::{CoupledPrimitive, Coupling},
};

pub(super) struct PropagationEdge {
    pub(super) source: usize,
    pub(super) destination: usize,
    pub(super) effect: CoupledPrimitive,
    pub(super) lag: Option<CoupledPrimitive>,
}

pub(super) struct InterventionEdge {
    pub(super) destination: usize,
    pub(super) effect: CoupledPrimitive,
    pub(super) rebound: Option<Distribution>,
    pub(super) profile: EffectProfile,
    pub(super) lag: Option<CoupledPrimitive>,
}

pub(super) fn propagation(
    edges: &[Edge],
    indices: &BTreeMap<EntityId, usize>,
    coupling: &Coupling,
) -> Result<Vec<PropagationEdge>, ScenarioAnalysisError> {
    edges
        .iter()
        .filter(|edge| {
            indices.contains_key(&edge.source) && indices.contains_key(&edge.destination)
        })
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Contributes(effect) => Some((
                edge,
                effect.response.id,
                &effect.response.distribution,
                effect.lag.as_ref().map(|lag| (lag.id, &lag.distribution)),
            )),
            EdgePayload::Blocks(effect) if edge.destination_kind != NodeKind::Intervention => {
                Some((edge, effect.degree.id, &effect.degree.distribution, None))
            }
            _ => None,
        })
        .map(|(edge, effect, distribution, lag)| {
            let owner = EstimateOwner::Edge(edge.id());
            Ok(PropagationEdge {
                source: indices[&edge.source],
                destination: indices[&edge.destination],
                effect: coupling.primitive(&owner, effect, distribution),
                lag: lag.map(|(id, lag)| coupling.primitive(&owner, id, lag)),
            })
        })
        .collect()
}

pub(super) fn intervention(
    candidate: EntityId,
    edges: &[Edge],
    indices: &BTreeMap<EntityId, usize>,
    coupling: &Coupling,
) -> Vec<InterventionEdge> {
    edges
        .iter()
        .filter_map(|edge| match &edge.payload {
            EdgePayload::Changes(effect)
                if edge.source == candidate && indices.contains_key(&edge.destination) =>
            {
                let owner = EstimateOwner::Edge(edge.id());
                let coupled = |id: EstimateId, distribution: &Distribution| {
                    coupling.primitive(&owner, id, distribution)
                };
                Some(InterventionEdge {
                    destination: indices[&edge.destination],
                    effect: coupled(effect.response.id, &effect.response.distribution),
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
                    lag: effect
                        .lag
                        .as_ref()
                        .map(|lag| coupled(lag.id, &lag.distribution)),
                })
            }
            _ => None,
        })
        .collect()
}
