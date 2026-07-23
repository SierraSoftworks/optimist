use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    Distribution, Estimate, EstimateAddress, EstimateDimension, EstimateSource,
    EstimateUncertainty, FermiEstimateSupport, Unit,
};

/// Semantic field within a node or edge payload where an estimate is embedded.
///
/// A slot is required when creating an optional estimate because an address only
/// identifies an estimate after its owner-local ID exists. Cost dimensions are
/// named rather than indexed so reordering a cost vector cannot retarget a command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EstimateSlot {
    /// Current normalized state of an outcome/factor or native value of a metric.
    Current,
    /// Desired normalized state of an outcome or factor.
    Desired,
    /// One named non-negative intervention cost dimension.
    Cost(String),
    /// Non-negative intervention completion duration.
    Duration,
    /// Probability that an intervention succeeds.
    ProbabilityOfSuccess,
    /// Signed causal effect of a contributes or changes edge.
    Effect,
    /// Native-unit destination change in a unit-aware linear response.
    Response,
    /// Non-negative lag of a contributes or changes edge.
    Lag,
    /// Signed blocking degree of a blocks edge.
    Degree,
}

impl EstimateSlot {
    /// Validates and canonicalizes user-provided slot data.
    pub fn validated(self) -> Result<Self, EstimateSlotError> {
        match self {
            Self::Cost(dimension) => {
                let dimension = dimension.trim().to_owned();
                if dimension.is_empty() {
                    Err(EstimateSlotError::EmptyCostDimension)
                } else {
                    Ok(Self::Cost(dimension))
                }
            }
            slot => Ok(slot),
        }
    }

    /// Returns the primitive support required by this semantic slot.
    pub fn fermi_support(&self) -> FermiEstimateSupport {
        match self {
            Self::Current | Self::Desired | Self::ProbabilityOfSuccess => {
                FermiEstimateSupport::Probability
            }
            Self::Effect | Self::Degree => FermiEstimateSupport::Signed,
            Self::Response => FermiEstimateSupport::Real,
            Self::Cost(_) | Self::Duration | Self::Lag => FermiEstimateSupport::NonNegative,
        }
    }

    /// Returns the runtime unit required by this semantic slot.
    pub fn unit(&self) -> Result<Unit, EstimateSlotError> {
        match self {
            Self::Cost(dimension) => {
                Unit::base(dimension).map_err(|_| EstimateSlotError::InvalidUnit(dimension.clone()))
            }
            Self::Duration | Self::Lag => Ok(Unit::base("duration").expect("valid unit")),
            Self::Current
            | Self::Desired
            | Self::ProbabilityOfSuccess
            | Self::Effect
            | Self::Response
            | Self::Degree => Ok(Unit::dimensionless()),
        }
    }
}

/// A dimension-neutral root estimate returned by API and CLI lookups.
///
/// The owning slot carries the semantic dimension. This transport form avoids
/// erasing validation in stored `Estimate<T>` values while permitting one output
/// schema for all typed owner fields.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PrimitiveEstimate {
    /// Stable project-scoped address of the embedded estimate.
    pub address: EstimateAddress,
    /// Semantic owner field whose type constrains distribution support.
    pub slot: EstimateSlot,
    /// Aggregate-local estimate revision.
    pub revision: u64,
    /// Validated primitive probability distribution.
    pub distribution: Distribution,
    /// Active authoring source and retained assessment when formula-derived.
    pub source: EstimateSource,
    /// Evidence or elicitation records supporting the estimate.
    pub provenance: Vec<String>,
    /// Distinct uncertainty sources retained without assuming independence.
    #[serde(default, skip_serializing_if = "EstimateUncertainty::is_empty")]
    pub uncertainty: EstimateUncertainty,
}

impl PrimitiveEstimate {
    /// Projects a typed stored estimate into the common API and CLI representation.
    pub fn from_typed<T: EstimateDimension>(
        address: EstimateAddress,
        slot: EstimateSlot,
        estimate: &Estimate<T>,
    ) -> Self {
        Self {
            address,
            slot,
            revision: estimate.revision,
            distribution: estimate.distribution.clone(),
            source: estimate.source.clone(),
            provenance: estimate.provenance.clone(),
            uncertainty: estimate.uncertainty.clone(),
        }
    }
}

/// Validation failures for semantic estimate slots.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateSlotError {
    /// Intervention costs require a nonempty project-defined dimension name.
    #[error("an intervention cost dimension cannot be empty")]
    EmptyCostDimension,
    /// A cost dimension cannot form a runtime unit identifier.
    #[error("invalid estimate unit {0:?}")]
    InvalidUnit(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn cost_slots_are_tagged_and_canonicalized() {
        let slot = EstimateSlot::Cost("  engineer_days ".to_owned())
            .validated()
            .unwrap();
        assert_eq!(slot, EstimateSlot::Cost("engineer_days".to_owned()));
        assert_eq!(
            serde_json::to_value(slot).unwrap(),
            json!({"kind": "cost", "value": "engineer_days"})
        );
        assert_eq!(
            EstimateSlot::Cost(" ".to_owned()).validated(),
            Err(EstimateSlotError::EmptyCostDimension)
        );
    }
}
