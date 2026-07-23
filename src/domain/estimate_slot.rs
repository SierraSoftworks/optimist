use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{
    Distribution, Estimate, EstimateAddress, EstimateDimension, EstimateSource,
    EstimateUncertainty, QuantityDefinition, SquiggleEstimateSupport, Unit,
    assess_squiggle_estimate,
};

/// Semantic field within a node or edge payload where an estimate is embedded.
///
/// A slot is required when creating an optional estimate because an address only
/// identifies an estimate after its owner-local ID exists. Cost dimensions are
/// named rather than indexed so reordering a cost vector cannot retarget a command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EstimateSlot {
    /// Current native value of a factor, outcome, or metric.
    Current,
    /// Forecast native value of an outcome or factor before interventions.
    Forecast,
    /// One named non-negative intervention cost dimension.
    Cost(String),
    /// Non-negative intervention completion duration.
    Duration,
    /// Probability that an intervention succeeds.
    ProbabilityOfSuccess,
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
    pub fn estimate_support(&self) -> SquiggleEstimateSupport {
        match self {
            Self::Current | Self::Forecast | Self::ProbabilityOfSuccess => {
                SquiggleEstimateSupport::Probability
            }
            Self::Degree => SquiggleEstimateSupport::Signed,
            Self::Response => SquiggleEstimateSupport::Real,
            Self::Cost(_) | Self::Duration | Self::Lag => SquiggleEstimateSupport::NonNegative,
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
            | Self::Forecast
            | Self::ProbabilityOfSuccess
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
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrimitiveEstimate {
    /// Stable project-scoped address of the embedded estimate.
    pub address: EstimateAddress,
    /// Semantic owner field whose type constrains distribution support.
    pub slot: EstimateSlot,
    /// Aggregate-local estimate revision.
    pub revision: u64,
    /// Runtime distribution derived from [`Self::source`]; never serialized.
    #[serde(skip_serializing)]
    pub distribution: Distribution,
    /// Intrinsic quantity definition when the estimate dimension owns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<QuantityDefinition>,
    /// Active Squiggle authoring source and deterministic controls.
    pub source: EstimateSource,
    /// Evidence or elicitation records supporting the estimate.
    pub provenance: Vec<String>,
    /// Distinct uncertainty sources retained without assuming independence.
    #[serde(default, skip_serializing_if = "EstimateUncertainty::is_empty")]
    pub uncertainty: EstimateUncertainty,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPrimitiveEstimate {
    address: EstimateAddress,
    slot: EstimateSlot,
    revision: u64,
    #[serde(default)]
    quantity: Option<QuantityDefinition>,
    source: EstimateSource,
    #[serde(default)]
    provenance: Vec<String>,
    #[serde(default)]
    uncertainty: EstimateUncertainty,
}

impl<'de> Deserialize<'de> for PrimitiveEstimate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawPrimitiveEstimate::deserialize(deserializer)?;
        let EstimateSource::Squiggle { definition } = raw.source;
        let target_unit = definition.target_unit.clone();
        let (definition, _, distribution) =
            assess_squiggle_estimate(*definition, &target_unit).map_err(de::Error::custom)?;
        Ok(Self {
            address: raw.address,
            slot: raw.slot,
            revision: raw.revision,
            distribution,
            quantity: raw.quantity,
            source: EstimateSource::Squiggle {
                definition: Box::new(definition),
            },
            provenance: raw.provenance,
            uncertainty: raw.uncertainty,
        })
    }
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
            quantity: estimate.quantity.clone(),
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

    #[test]
    fn primitive_estimates_round_trip_from_source_without_distributions() {
        let project = crate::domain::ProjectId::new("A").unwrap();
        let estimate = Estimate::<crate::domain::Probability>::from_squiggle(
            crate::domain::EstimateId::new(0),
            crate::domain::SquiggleEstimateDefinition {
                source: "beta(8, 2)".to_owned(),
                seed: 42,
                sample_count: 256,
                target_unit: Unit::dimensionless(),
            },
            &Unit::dimensionless(),
        )
        .unwrap();
        let primitive = PrimitiveEstimate::from_typed(
            EstimateAddress::new(
                project,
                crate::domain::EstimateOwner::Node(crate::domain::EntityId::new(0)),
                estimate.id,
            ),
            EstimateSlot::Current,
            &estimate,
        );
        let value = serde_json::to_value(&primitive).unwrap();
        assert!(value.get("distribution").is_none());
        assert_eq!(
            serde_json::from_value::<PrimitiveEstimate>(value).unwrap(),
            primitive
        );
    }
}
