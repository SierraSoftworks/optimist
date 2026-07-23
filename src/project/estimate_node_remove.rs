use crate::domain::{EstimateAddress, EstimateSlot, Node, NodePayload, PrimitiveEstimate};

use super::{ProjectError, estimate_node_find, estimate_support};

pub(super) fn remove(
    node: &mut Node,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    let existing = estimate_node_find::find(node, address)?;
    if let Some(state) = &mut node.native_state {
        match existing.slot {
            EstimateSlot::Current => state.current = None,
            EstimateSlot::Desired => state.forecast = None,
            _ => return Err(estimate_support::invalid_slot(address, existing.slot)),
        }
        return Ok(existing);
    }
    match (&mut node.payload, &existing.slot) {
        (NodePayload::Outcome(value), EstimateSlot::Current) => value.current = None,
        (NodePayload::Outcome(value), EstimateSlot::Desired) => value.desired = None,
        (NodePayload::Factor(value), EstimateSlot::Current) => value.current = None,
        (NodePayload::Factor(value), EstimateSlot::Desired) => value.desired = None,
        (NodePayload::Metric(value), EstimateSlot::Current) => value.current = None,
        (NodePayload::Intervention(value), EstimateSlot::Cost(_)) => {
            value.costs.retain(|cost| cost.value.id != address.estimate);
        }
        (NodePayload::Intervention(value), EstimateSlot::Duration) => value.duration = None,
        (NodePayload::Intervention(value), EstimateSlot::ProbabilityOfSuccess) => {
            value.probability_of_success = None;
        }
        _ => return Err(estimate_support::invalid_slot(address, existing.slot)),
    }
    Ok(existing)
}
