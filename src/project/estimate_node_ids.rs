use crate::domain::{EstimateId, Node, NodePayload};

pub(super) fn count(node: &Node, id: EstimateId) -> usize {
    let native = node.native_state.as_ref().map_or(0, |state| {
        quantity_state(&state.current, &state.forecast, id)
    });
    native
        + match &node.payload {
            NodePayload::Outcome(_) | NodePayload::Factor(_) => 0,
            NodePayload::Intervention(value) => {
                value
                    .costs
                    .iter()
                    .filter(|cost| cost.value.id == id)
                    .count()
                    + usize::from(value.duration.as_ref().is_some_and(|item| item.id == id))
                    + usize::from(
                        value
                            .probability_of_success
                            .as_ref()
                            .is_some_and(|item| item.id == id),
                    )
            }
            NodePayload::Metric(value) => {
                usize::from(value.current.as_ref().is_some_and(|item| item.id == id))
            }
        }
}

fn quantity_state(
    current: &Option<crate::domain::Estimate<crate::domain::QuantityValue>>,
    forecast: &Option<crate::domain::Estimate<crate::domain::QuantityValue>>,
    id: EstimateId,
) -> usize {
    [current.as_ref(), forecast.as_ref()]
        .into_iter()
        .flatten()
        .filter(|item| item.id == id)
        .count()
}
