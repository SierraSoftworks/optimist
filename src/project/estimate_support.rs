use crate::domain::{
    Distribution, Estimate, EstimateAddress, EstimateDimension, EstimateSlot, EstimateSource,
    PrimitiveEstimate,
};

use super::{EstimateCommandError, ProjectError};

pub(super) fn replacement<T: EstimateDimension>(
    current: Option<&Estimate<T>>,
    address: &EstimateAddress,
    slot: EstimateSlot,
    owner_id_count: usize,
    distribution: Distribution,
    source: EstimateSource,
    provenance: Vec<String>,
) -> Result<(Estimate<T>, PrimitiveEstimate), ProjectError> {
    let revision = match current {
        Some(estimate) if estimate.id != address.estimate => {
            return Err(EstimateCommandError::SlotOccupied {
                address: address.clone(),
                slot,
            }
            .into());
        }
        Some(_) if owner_id_count != 1 => {
            return Err(EstimateCommandError::IdentifierConflict(address.clone()).into());
        }
        Some(estimate) => estimate
            .revision
            .checked_add(1)
            .ok_or_else(|| EstimateCommandError::RevisionSpaceExhausted(address.clone()))?,
        None if owner_id_count != 0 => {
            return Err(EstimateCommandError::IdentifierConflict(address.clone()).into());
        }
        None => 0,
    };
    let mut estimate =
        Estimate::<T>::new(address.estimate, distribution).map_err(EstimateCommandError::from)?;
    estimate.revision = revision;
    estimate.source = source;
    estimate.provenance = provenance;
    let value = PrimitiveEstimate::from_typed(address.clone(), slot, &estimate);
    Ok((estimate, value))
}

pub(super) fn invalid_slot(address: &EstimateAddress, slot: EstimateSlot) -> ProjectError {
    EstimateCommandError::InvalidSlot {
        address: address.clone(),
        slot,
    }
    .into()
}
