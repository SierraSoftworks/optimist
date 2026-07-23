use std::collections::BTreeMap;

use crate::domain::{
    EdgePayload, EntityId, EstimateAddress, EstimateId, EstimateOwner, NodePayload,
};

use super::{EntityDocument, ImportError, ProjectDocument, SourceDocument};

pub(super) fn validate(
    project: &SourceDocument<ProjectDocument>,
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
) -> Result<(), ImportError> {
    let Some(model) = &project.document.dependence else {
        return Ok(());
    };
    for address in model
        .residual_groups
        .iter()
        .flat_map(|group| &group.members)
    {
        if address.project != project.document.project.id || !contains_estimate(entities, address) {
            return Err(ImportError::MissingDependenceEstimate {
                path: project.path.clone(),
                address: address.clone(),
            });
        }
    }
    Ok(())
}

fn contains_estimate(
    entities: &BTreeMap<EntityId, SourceDocument<EntityDocument>>,
    address: &EstimateAddress,
) -> bool {
    match &address.owner {
        EstimateOwner::Node(id) => entities
            .get(id)
            .is_some_and(|node| node_contains(&node.document.node, address.estimate)),
        EstimateOwner::Edge(id) => entities
            .get(&id.source)
            .and_then(|node| {
                node.document
                    .outgoing_edges
                    .iter()
                    .find(|edge| edge.id() == *id)
            })
            .is_some_and(|edge| edge_contains(&edge.payload, address.estimate)),
    }
}

fn node_contains(node: &crate::domain::Node, id: EstimateId) -> bool {
    let state_contains = node.native_state.as_ref().is_some_and(|state| {
        matches_estimate(&state.current, id) || matches_estimate(&state.forecast, id)
    });
    state_contains
        || match &node.payload {
            NodePayload::Outcome(_) | NodePayload::Factor(_) => false,
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
            value.response.destination_change.id == id || matches_estimate(&value.lag, id)
        }
        EdgePayload::Blocks(value) => value.degree.id == id,
        _ => false,
    }
}
