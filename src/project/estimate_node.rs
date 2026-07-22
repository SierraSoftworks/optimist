use crate::domain::{
    CostEstimate, Distribution, EstimateAddress, EstimateSlot, EstimateSource, Intervention, Node,
    NodePayload, PrimitiveEstimate,
};

use super::{EstimateCommandError, ProjectError, estimate_node_ids, estimate_support};

pub(super) fn set(
    node: &mut Node,
    address: &EstimateAddress,
    slot: EstimateSlot,
    distribution: Distribution,
    source: EstimateSource,
    provenance: Vec<String>,
) -> Result<PrimitiveEstimate, ProjectError> {
    let count = estimate_node_ids::count(&node.payload, address.estimate);
    match (&mut node.payload, slot.clone()) {
        (NodePayload::Outcome(value), EstimateSlot::Current) => estimate_support::replacement(
            value.current.as_ref(),
            address,
            slot,
            count,
            distribution,
            source,
            provenance,
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
            source,
            provenance,
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
            source,
            provenance,
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
            source,
            provenance,
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
            source,
            provenance,
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
        (NodePayload::Intervention(value), EstimateSlot::Cost(dimension)) => set_cost(
            value,
            address,
            dimension,
            count,
            distribution,
            source,
            provenance,
        ),
        (NodePayload::Intervention(value), EstimateSlot::Duration) => {
            estimate_support::replacement(
                value.duration.as_ref(),
                address,
                slot,
                count,
                distribution,
                source,
                provenance,
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
                source,
                provenance,
            )
            .map(|(estimate, result)| {
                value.probability_of_success = Some(estimate);
                result
            })
        }
        _ => Err(estimate_support::invalid_slot(address, slot)),
    }
}

fn set_cost(
    value: &mut Intervention,
    address: &EstimateAddress,
    dimension: String,
    count: usize,
    distribution: Distribution,
    source: EstimateSource,
    provenance: Vec<String>,
) -> Result<PrimitiveEstimate, ProjectError> {
    let index = value
        .costs
        .iter()
        .position(|cost| cost.dimension == dimension);
    let current = index.map(|index| &value.costs[index].value);
    let slot = EstimateSlot::Cost(dimension.clone());
    let (estimate, result) = estimate_support::replacement(
        current,
        address,
        slot,
        count,
        distribution,
        source,
        provenance,
    )?;
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
