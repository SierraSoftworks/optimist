use crate::domain::{
    CostEstimate, Distribution, EstimateAddress, EstimateSlot, Intervention, Node, NodePayload,
    PrimitiveEstimate,
};

use super::{
    EstimateCommandError, ProjectError, estimate_node_ids,
    estimate_support::{self, EstimateMetadata},
};

pub(super) fn set(
    node: &mut Node,
    address: &EstimateAddress,
    slot: EstimateSlot,
    distribution: Distribution,
    metadata: EstimateMetadata,
) -> Result<PrimitiveEstimate, ProjectError> {
    let count = estimate_node_ids::count(node, address.estimate);
    if node.native_state.is_some() && matches!(slot, EstimateSlot::Current | EstimateSlot::Desired)
    {
        return set_native(node, address, slot, count, distribution, metadata);
    }
    match (&mut node.payload, slot.clone()) {
        (NodePayload::Outcome(value), EstimateSlot::Current) => estimate_support::replacement(
            value.current.as_ref(),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .map(|(estimate, result)| {
            value.current = Some(estimate);
            result
        }),
        (NodePayload::Outcome(value), EstimateSlot::Desired) => estimate_support::replacement(
            value.desired.as_ref(),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .map(|(estimate, result)| {
            value.desired = Some(estimate);
            result
        }),
        (NodePayload::Factor(value), EstimateSlot::Current) => estimate_support::replacement(
            value.current.as_ref(),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .map(|(estimate, result)| {
            value.current = Some(estimate);
            result
        }),
        (NodePayload::Factor(value), EstimateSlot::Desired) => estimate_support::replacement(
            value.desired.as_ref(),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .map(|(estimate, result)| {
            value.desired = Some(estimate);
            result
        }),
        (NodePayload::Metric(value), EstimateSlot::Current) => estimate_support::replacement(
            value.current.as_ref(),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .and_then(|(estimate, result)| {
            if !value.quantity.accepts(&estimate.distribution) {
                return Err(EstimateCommandError::Quantity(
                    crate::domain::QuantityError::EstimateOutsideSupport,
                )
                .into());
            }
            value.current = Some(estimate);
            Ok(result)
        }),
        (NodePayload::Intervention(value), EstimateSlot::Cost(dimension)) => {
            set_cost(value, address, dimension, count, distribution, metadata)
        }
        (NodePayload::Intervention(value), EstimateSlot::Duration) => {
            estimate_support::replacement(
                value.duration.as_ref(),
                address,
                slot,
                count,
                distribution,
                metadata,
            )
            .map(|(estimate, result)| {
                value.duration = Some(estimate);
                result
            })
        }
        (NodePayload::Intervention(value), EstimateSlot::ProbabilityOfSuccess) => {
            estimate_support::replacement(
                value.probability_of_success.as_ref(),
                address,
                slot,
                count,
                distribution,
                metadata,
            )
            .map(|(estimate, result)| {
                value.probability_of_success = Some(estimate);
                result
            })
        }
        _ => Err(estimate_support::invalid_slot(address, slot)),
    }
}

fn set_native(
    node: &mut Node,
    address: &EstimateAddress,
    slot: EstimateSlot,
    count: usize,
    distribution: Distribution,
    metadata: EstimateMetadata,
) -> Result<PrimitiveEstimate, ProjectError> {
    let state = node.native_state.as_mut().expect("native state checked");
    let current = match slot {
        EstimateSlot::Current => state.current.as_ref(),
        EstimateSlot::Desired => state.forecast.as_ref(),
        _ => return Err(estimate_support::invalid_slot(address, slot)),
    };
    let result_slot = slot.clone();
    let (mut estimate, _) =
        estimate_support::replacement(current, address, slot, count, distribution, metadata)?;
    if !state.quantity.accepts(&estimate.distribution) {
        return Err(crate::domain::QuantityError::EstimateOutsideSupport.into());
    }
    estimate.quantity = Some(state.quantity.clone());
    let result = PrimitiveEstimate::from_typed(address.clone(), result_slot.clone(), &estimate);
    match result_slot {
        EstimateSlot::Current => state.current = Some(estimate),
        EstimateSlot::Desired => state.forecast = Some(estimate),
        _ => unreachable!("native slots checked"),
    }
    Ok(result)
}

fn set_cost(
    value: &mut Intervention,
    address: &EstimateAddress,
    dimension: String,
    count: usize,
    distribution: Distribution,
    metadata: EstimateMetadata,
) -> Result<PrimitiveEstimate, ProjectError> {
    let index = value
        .costs
        .iter()
        .position(|cost| cost.dimension == dimension);
    let current = index.map(|index| &value.costs[index].value);
    let slot = EstimateSlot::Cost(dimension.clone());
    let (estimate, result) =
        estimate_support::replacement(current, address, slot, count, distribution, metadata)?;
    if let Some(index) = index {
        value.costs[index].value = estimate;
    } else {
        value.costs.push(CostEstimate {
            dimension,
            value: estimate,
        });
    }
    Ok(result)
}
