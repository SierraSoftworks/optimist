use crate::{
    domain::{
        EdgePayload, EstimateAddress, EstimateId, EstimateOwner, NodePayload,
        ProjectDependenceModel,
    },
    store::GraphRepository,
};

use super::{ProjectError, catalog::ProjectEntry};

pub(super) fn validate(
    entry: &mut ProjectEntry,
    model: &ProjectDependenceModel,
) -> Result<(), ProjectError> {
    for address in model
        .residual_groups
        .iter()
        .flat_map(|group| &group.members)
    {
        if !address.components.is_empty() || !contains(entry, address)? {
            return Err(ProjectError::MissingEstimateAddress(address.clone()));
        }
    }
    Ok(())
}

fn contains(entry: &mut ProjectEntry, address: &EstimateAddress) -> Result<bool, ProjectError> {
    match &address.owner {
        EstimateOwner::Node(id) => Ok(entry
            .repository
            .get_node(*id)?
            .is_some_and(|node| node_contains(&node.payload, address.estimate))),
        EstimateOwner::Edge(id) => Ok(entry
            .repository
            .get_edge(id)?
            .is_some_and(|edge| edge_contains(&edge.payload, address.estimate))),
    }
}

fn node_contains(payload: &NodePayload, id: EstimateId) -> bool {
    match payload {
        NodePayload::Outcome(value) => {
            matches_estimate(&value.current, id) || matches_estimate(&value.desired, id)
        }
        NodePayload::Factor(value) => {
            matches_estimate(&value.current, id) || matches_estimate(&value.desired, id)
        }
        NodePayload::Intervention(value) => {
            value.costs.iter().any(|cost| cost.value.id == id)
                || matches_estimate(&value.duration, id)
                || matches_estimate(&value.probability_of_success, id)
        }
        NodePayload::Metric(_) => false,
    }
}

fn matches_estimate<T: crate::domain::EstimateDimension>(
    estimate: &Option<crate::domain::Estimate<T>>,
    id: EstimateId,
) -> bool {
    estimate.as_ref().is_some_and(|estimate| estimate.id == id)
}

fn edge_contains(payload: &EdgePayload, id: EstimateId) -> bool {
    match payload {
        EdgePayload::Contributes(value) | EdgePayload::Changes(value) => {
            value.effect.id == id || matches_estimate(&value.lag, id)
        }
        EdgePayload::Blocks(value) => value.degree.id == id,
        _ => false,
    }
}
