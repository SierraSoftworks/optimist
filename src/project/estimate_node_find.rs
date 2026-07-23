use crate::domain::{
    EstimateAddress, EstimateSlot, Intervention, Node, NodePayload, PrimitiveEstimate,
};

use super::{EstimateCommandError, ProjectError, estimate_node_ids};

pub(super) fn find(
    node: &Node,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    if estimate_node_ids::count(node, address.estimate) != 1 {
        return Err(EstimateCommandError::NotFound(address.clone()).into());
    }
    let state = node
        .native_state
        .as_ref()
        .and_then(|state| quantity_state(address, &state.current, &state.forecast));
    match &node.payload {
        NodePayload::Outcome(_) | NodePayload::Factor(_) => state,
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

fn quantity_state(
    address: &EstimateAddress,
    current: &Option<crate::domain::Estimate<crate::domain::QuantityValue>>,
    forecast: &Option<crate::domain::Estimate<crate::domain::QuantityValue>>,
) -> Option<PrimitiveEstimate> {
    current
        .as_ref()
        .filter(|value| value.id == address.estimate)
        .map(|value| PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Current, value))
        .or_else(|| {
            forecast
                .as_ref()
                .filter(|value| value.id == address.estimate)
                .map(|value| {
                    PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Forecast, value)
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
