use crate::domain::{
    EstimateAddress, EstimateSlot, Intervention, Node, NodePayload, PrimitiveEstimate,
};

use super::{EstimateCommandError, ProjectError, estimate_node_ids};

pub(super) fn find(
    node: &Node,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    if estimate_node_ids::count(&node.payload, address.estimate) != 1 {
        return Err(EstimateCommandError::NotFound(address.clone()).into());
    }
    match &node.payload {
        NodePayload::Outcome(value) => state(address, &value.current, &value.desired),
        NodePayload::Factor(value) => state(address, &value.current, &value.desired),
        NodePayload::Intervention(value) => intervention(address, value),
        NodePayload::Metric(value) => value
            .current
            .as_ref()
            .filter(|item| item.id == address.estimate)
            .map(|item| {
                PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Current, item)
            }),
    }
    .ok_or_else(|| EstimateCommandError::NotFound(address.clone()).into())
}

fn state(
    address: &EstimateAddress,
    current: &Option<crate::domain::Estimate<crate::domain::NormalizedState>>,
    desired: &Option<crate::domain::Estimate<crate::domain::NormalizedState>>,
) -> Option<PrimitiveEstimate> {
    current
        .as_ref()
        .filter(|value| value.id == address.estimate)
        .map(|value| PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Current, value))
        .or_else(|| {
            desired
                .as_ref()
                .filter(|value| value.id == address.estimate)
                .map(|value| {
                    PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Desired, value)
                })
        })
}

fn intervention(address: &EstimateAddress, value: &Intervention) -> Option<PrimitiveEstimate> {
    value
        .costs
        .iter()
        .find(|cost| cost.value.id == address.estimate)
        .map(|cost| {
            PrimitiveEstimate::from_typed(
                address.clone(),
                EstimateSlot::Cost(cost.dimension.clone()),
                &cost.value,
            )
        })
        .or_else(|| {
            value
                .duration
                .as_ref()
                .filter(|item| item.id == address.estimate)
                .map(|item| {
                    PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Duration, item)
                })
        })
        .or_else(|| {
            value
                .probability_of_success
                .as_ref()
                .filter(|item| item.id == address.estimate)
                .map(|item| {
                    PrimitiveEstimate::from_typed(
                        address.clone(),
                        EstimateSlot::ProbabilityOfSuccess,
                        item,
                    )
                })
        })
}
