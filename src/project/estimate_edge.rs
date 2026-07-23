use crate::domain::{
    Distribution, Edge, EdgePayload, EstimateAddress, EstimateId, EstimateSlot, PrimitiveEstimate,
};

use super::{
    EstimateCommandError, ProjectError,
    estimate_support::{self, EstimateMetadata},
};

pub(super) fn set(
    edge: &mut Edge,
    address: &EstimateAddress,
    slot: EstimateSlot,
    distribution: Distribution,
    metadata: EstimateMetadata,
) -> Result<PrimitiveEstimate, ProjectError> {
    let count = count_id(&edge.payload, address.estimate);
    match (&mut edge.payload, slot.clone()) {
        (EdgePayload::Contributes(value) | EdgePayload::Changes(value), EstimateSlot::Effect) => {
            let Some(current) = value.normalized_effect() else {
                return Err(estimate_support::invalid_slot(address, slot));
            };
            estimate_support::replacement(
                Some(current),
                address,
                slot,
                count,
                distribution,
                metadata,
            )
            .map(|(estimate, result)| {
                *value.normalized_effect_mut().expect("model checked") = estimate;
                result
            })
        }
        (EdgePayload::Contributes(value), EstimateSlot::Response) => {
            let Some(current) = value.linear_response().map(|item| &item.destination_change) else {
                return Err(estimate_support::invalid_slot(address, slot));
            };
            estimate_support::replacement(
                Some(current),
                address,
                slot,
                count,
                distribution,
                metadata,
            )
            .map(|(estimate, result)| {
                value
                    .linear_response_mut()
                    .expect("model checked")
                    .destination_change = estimate;
                result
            })
        }
        (EdgePayload::Contributes(value) | EdgePayload::Changes(value), EstimateSlot::Lag) => {
            estimate_support::replacement(
                value.lag.as_ref(),
                address,
                slot,
                count,
                distribution,
                metadata,
            )
            .map(|(estimate, result)| {
                value.lag = Some(estimate);
                result
            })
        }
        (EdgePayload::Blocks(value), EstimateSlot::Degree) => estimate_support::replacement(
            Some(&value.degree),
            address,
            slot,
            count,
            distribution,
            metadata,
        )
        .map(|(estimate, result)| {
            value.degree = estimate;
            result
        }),
        _ => Err(estimate_support::invalid_slot(address, slot)),
    }
}

pub(super) fn find(
    edge: &Edge,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    if count_id(&edge.payload, address.estimate) != 1 {
        return Err(EstimateCommandError::NotFound(address.clone()).into());
    }
    match &edge.payload {
        EdgePayload::Contributes(value) | EdgePayload::Changes(value) => {
            if let Some(effect) = value
                .normalized_effect()
                .filter(|effect| effect.id == address.estimate)
            {
                Some(PrimitiveEstimate::from_typed(
                    address.clone(),
                    EstimateSlot::Effect,
                    effect,
                ))
            } else if let Some(response) = value
                .linear_response()
                .filter(|response| response.destination_change.id == address.estimate)
            {
                Some(PrimitiveEstimate::from_typed(
                    address.clone(),
                    EstimateSlot::Response,
                    &response.destination_change,
                ))
            } else {
                value
                    .lag
                    .as_ref()
                    .filter(|item| item.id == address.estimate)
                    .map(|item| {
                        PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Lag, item)
                    })
            }
        }
        EdgePayload::Blocks(value) if value.degree.id == address.estimate => Some(
            PrimitiveEstimate::from_typed(address.clone(), EstimateSlot::Degree, &value.degree),
        ),
        _ => None,
    }
    .ok_or_else(|| EstimateCommandError::NotFound(address.clone()).into())
}

pub(super) fn remove(
    edge: &mut Edge,
    address: &EstimateAddress,
) -> Result<PrimitiveEstimate, ProjectError> {
    let existing = find(edge, address)?;
    match (&mut edge.payload, &existing.slot) {
        (EdgePayload::Contributes(value) | EdgePayload::Changes(value), EstimateSlot::Lag) => {
            value.lag = None;
            Ok(existing)
        }
        (_, EstimateSlot::Effect | EstimateSlot::Response | EstimateSlot::Degree) => {
            Err(EstimateCommandError::Required {
                address: address.clone(),
                slot: existing.slot,
            }
            .into())
        }
        _ => Err(estimate_support::invalid_slot(address, existing.slot)),
    }
}

fn count_id(payload: &EdgePayload, id: EstimateId) -> usize {
    match payload {
        EdgePayload::Contributes(value) | EdgePayload::Changes(value) => {
            usize::from(
                value
                    .normalized_effect()
                    .is_some_and(|effect| effect.id == id),
            ) + usize::from(
                value
                    .linear_response()
                    .is_some_and(|response| response.destination_change.id == id),
            ) + usize::from(value.lag.as_ref().is_some_and(|item| item.id == id))
        }
        EdgePayload::Blocks(value) => usize::from(value.degree.id == id),
        _ => 0,
    }
}
