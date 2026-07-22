use crate::domain::{EstimateId, NodePayload};

pub(super) fn count(payload: &NodePayload, id: EstimateId) -> usize {
    match payload {
        NodePayload::Outcome(value) => state(&value.current, &value.desired, id),
        NodePayload::Factor(value) => state(&value.current, &value.desired, id),
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

fn state(
    current: &Option<crate::domain::Estimate<crate::domain::NormalizedState>>,
    desired: &Option<crate::domain::Estimate<crate::domain::NormalizedState>>,
    id: EstimateId,
) -> usize {
    [current.as_ref(), desired.as_ref()]
        .into_iter()
        .flatten()
        .filter(|item| item.id == id)
        .count()
}
