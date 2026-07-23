use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{Distribution, FermiEstimateSupport, Unit};

const MAX_UNIT_BYTES: usize = 256;
const MAX_CONTEXT_BYTES: usize = 4_096;

/// Complete support expected for a native-unit quantity.
///
/// Support describes which values are physically or semantically possible; it does
/// not describe whether larger or smaller values are desirable. Scenario objectives
/// own that separate preference decision.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QuantitySupport {
    /// Any finite real-valued observation is meaningful.
    #[default]
    Real,
    /// Values below zero are impossible for this quantity.
    NonNegative,
    /// Values must remain inside the inclusive native-unit interval.
    Bounded {
        /// Smallest possible value in the quantity's native unit.
        lower: f64,
        /// Largest possible value in the quantity's native unit.
        upper: f64,
    },
}

impl QuantitySupport {
    pub(super) fn is_real(&self) -> bool {
        matches!(self, Self::Real)
    }

    pub(super) fn accepts(&self, distribution: &Distribution) -> bool {
        match self {
            Self::Real => true,
            Self::NonNegative => distribution.is_non_negative(),
            Self::Bounded { lower, upper } => distribution.is_within(*lower, *upper),
        }
    }

    fn validated(self) -> Result<Self, QuantityError> {
        match self {
            Self::Bounded { lower, upper }
                if !lower.is_finite() || !upper.is_finite() || lower >= upper =>
            {
                Err(QuantityError::InvalidBounds)
            }
            value => Ok(value),
        }
    }

    /// Returns the primitive family support used when assessing a Fermi source.
    pub fn fermi_support(self) -> FermiEstimateSupport {
        match self {
            Self::Real => FermiEstimateSupport::Real,
            Self::NonNegative => FermiEstimateSupport::NonNegative,
            Self::Bounded { lower, upper } => FermiEstimateSupport::Bounded { lower, upper },
        }
    }
}

/// Operational definition of a quantity expressed in its native unit.
///
/// This definition is descriptive rather than preferential. For example, deployment
/// frequency can remain measured in `deployments/week` while different scenarios
/// independently decide whether larger values are useful.
///
/// ```
/// use optimist::domain::{QuantityDefinition, QuantitySupport};
///
/// let quantity = QuantityDefinition::new(
///     "days",
///     Some("p95 over a calendar week".to_owned()),
///     QuantitySupport::NonNegative,
/// )?;
/// assert_eq!(quantity.unit, "days");
/// # Ok::<(), optimist::domain::QuantityError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuantityDefinition {
    /// Human-authored unit used by observations, estimates, and explanations.
    pub unit: String,
    /// Canonical unit terms used by typed formulas and Squiggle annotations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<Unit>,
    /// Optional aggregation and sampling window such as `p95 weekly`.
    pub aggregation: Option<String>,
    /// Complete legal support in the quantity's native unit.
    #[serde(default, skip_serializing_if = "QuantitySupport::is_real")]
    pub support: QuantitySupport,
    /// Resolvable description of exactly what is counted or measured.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub operational_definition: String,
    /// Optional timestamp, horizon, or period to which a forecast applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_time: Option<String>,
    /// Optional system, publication, query, or authority used to resolve the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_source: Option<String>,
}

#[derive(Deserialize)]
struct QuantityDefinitionWire {
    unit: String,
    #[serde(default)]
    dimension: Option<Unit>,
    aggregation: Option<String>,
    #[serde(default)]
    support: QuantitySupport,
    #[serde(default)]
    operational_definition: String,
    #[serde(default)]
    reference_time: Option<String>,
    #[serde(default)]
    resolution_source: Option<String>,
}

impl<'de> Deserialize<'de> for QuantityDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = QuantityDefinitionWire::deserialize(deserializer)?;
        Self {
            unit: value.unit,
            dimension: value.dimension,
            aggregation: value.aggregation,
            support: value.support,
            operational_definition: value.operational_definition,
            reference_time: value.reference_time,
            resolution_source: value.resolution_source,
        }
        .validated()
        .map_err(de::Error::custom)
    }
}

impl QuantityDefinition {
    /// Returns the canonical definition for legacy factor and outcome state.
    ///
    /// Existing estimates remain dimensionless values on `[0, 1]`; this metadata
    /// makes that inherited convention explicit without changing distributions or
    /// causal calculations.
    ///
    /// ```
    /// use optimist::domain::{QuantityDefinition, QuantitySupport};
    ///
    /// let quantity = QuantityDefinition::legacy_standardized_state();
    /// assert_eq!(quantity.unit, "standardized_state");
    /// assert_eq!(
    ///     quantity.support,
    ///     QuantitySupport::Bounded { lower: 0.0, upper: 1.0 },
    /// );
    /// ```
    pub fn legacy_standardized_state() -> Self {
        Self {
            unit: "standardized_state".to_owned(),
            dimension: Some(Unit::dimensionless()),
            aggregation: None,
            support: QuantitySupport::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            operational_definition: "Legacy standardized factor or outcome state where 0 and 1 are model-specific anchors.".to_owned(),
            reference_time: None,
            resolution_source: None,
        }
    }

    /// Creates a minimal validated quantity definition.
    pub fn new(
        unit: impl Into<String>,
        aggregation: Option<String>,
        support: QuantitySupport,
    ) -> Result<Self, QuantityError> {
        let unit = unit.into();
        let dimension = Unit::base(unit.trim()).ok();
        Self::with_dimension(unit, dimension, aggregation, support)
    }

    /// Creates a quantity with an explicit canonical unit expression.
    pub fn with_dimension(
        unit: impl Into<String>,
        dimension: Option<Unit>,
        aggregation: Option<String>,
        support: QuantitySupport,
    ) -> Result<Self, QuantityError> {
        Self {
            unit: unit.into(),
            dimension,
            aggregation,
            support,
            operational_definition: String::new(),
            reference_time: None,
            resolution_source: None,
        }
        .validated()
    }

    /// Validates unit, support, and bounded human-authored context fields.
    pub fn validated(mut self) -> Result<Self, QuantityError> {
        self.unit = self.unit.trim().to_owned();
        if self.unit.is_empty() {
            return Err(QuantityError::EmptyUnit);
        }
        if self.unit.len() > MAX_UNIT_BYTES {
            return Err(QuantityError::ContextTooLarge("unit"));
        }
        self.support = self.support.validated()?;
        for (name, value) in [
            ("aggregation", self.aggregation.as_deref()),
            (
                "operational definition",
                Some(self.operational_definition.as_str()),
            ),
            ("reference time", self.reference_time.as_deref()),
            ("resolution source", self.resolution_source.as_deref()),
        ] {
            if value.is_some_and(|value| value.len() > MAX_CONTEXT_BYTES) {
                return Err(QuantityError::ContextTooLarge(name));
            }
        }
        Ok(self)
    }

    /// Reports whether a distribution's complete support fits this quantity.
    pub fn accepts(&self, distribution: &Distribution) -> bool {
        self.support.accepts(distribution)
    }

    /// Returns the unit and support required to persist a Fermi estimate.
    pub fn fermi_target(&self) -> Result<(FermiEstimateSupport, Unit), QuantityError> {
        let dimension = self
            .dimension
            .clone()
            .ok_or(QuantityError::MissingDimension)?;
        Ok((self.support.fermi_support(), dimension))
    }
}

/// Invalid native quantity definitions or estimates.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum QuantityError {
    /// Every quantity requires a visible native unit.
    #[error("a quantity unit cannot be empty")]
    EmptyUnit,
    /// A human-authored definition field exceeded its transport bound.
    #[error("quantity {0} exceeds its maximum length")]
    ContextTooLarge(&'static str),
    /// Bounded support requires two ordered finite anchors.
    #[error("bounded quantity support requires finite lower < upper")]
    InvalidBounds,
    /// The current estimate assigns probability outside the quantity's legal support.
    #[error("quantity estimate support is incompatible with its definition")]
    EstimateOutsideSupport,
    /// Estimate metadata disagrees with its owner-defined quantity.
    #[error("estimate quantity metadata does not match its owning definition")]
    EstimateDefinitionMismatch,
    /// Legacy or externally authored quantity text has no canonical unit terms.
    #[error("quantity requires a canonical dimension before it can use typed formulas")]
    MissingDimension,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_definitions_during_construction_and_deserialization() {
        assert_eq!(
            QuantityDefinition::new(" ", None, QuantitySupport::Real),
            Err(QuantityError::EmptyUnit)
        );
        let json = r#"{
            "unit":"days",
            "aggregation":null,
            "support":{"type":"bounded","lower":10,"upper":5}
        }"#;
        assert!(serde_json::from_str::<QuantityDefinition>(json).is_err());
    }

    #[test]
    fn legacy_fields_default_to_real_support_without_serialization_churn() {
        let json = r#"{"unit":"days","aggregation":"p95 weekly"}"#;
        let quantity = serde_json::from_str::<QuantityDefinition>(json).unwrap();

        assert_eq!(quantity.support, QuantitySupport::Real);
        assert_eq!(quantity.dimension, None);
        assert_eq!(serde_json::to_string(&quantity).unwrap(), json);
    }

    #[test]
    fn explicit_dimensions_define_native_fermi_targets() {
        let dimension = Unit::from_exponents([("item", 1), ("day", -1)]).unwrap();
        let quantity = QuantityDefinition::with_dimension(
            "items/day",
            Some(dimension.clone()),
            None,
            QuantitySupport::Bounded {
                lower: 0.0,
                upper: 30.0,
            },
        )
        .unwrap();

        assert_eq!(
            quantity.fermi_target(),
            Ok((
                FermiEstimateSupport::Bounded {
                    lower: 0.0,
                    upper: 30.0,
                },
                dimension,
            ))
        );
    }
}
