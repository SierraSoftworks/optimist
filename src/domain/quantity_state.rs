use serde::{Deserialize, Deserializer, Serialize, de};

use super::{Estimate, QuantityDefinition, QuantityError, QuantityValue};

/// Native-unit current and forecast state owned by a factor or outcome node.
///
/// Legacy nodes omit this record and continue to use their standardized payload
/// estimates. Native state requires canonical unit terms and validates both
/// estimates against the same quantity support.
///
/// ```
/// use optimist::domain::{QuantityDefinition, QuantityState, QuantitySupport};
///
/// let state = QuantityState::new(
///     QuantityDefinition::new("days", None, QuantitySupport::NonNegative)?,
///     None,
///     None,
/// )?;
/// assert_eq!(state.quantity.unit, "days");
/// # Ok::<(), optimist::domain::QuantityError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct QuantityState {
    /// Operational definition, support, and canonical unit for both estimates.
    pub quantity: QuantityDefinition,
    /// Optional uncertain value at the quantity's reference time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<Estimate<QuantityValue>>,
    /// Optional uncertain future value before scenario interventions are applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast: Option<Estimate<QuantityValue>>,
}

#[derive(Deserialize)]
struct QuantityStateWire {
    quantity: QuantityDefinition,
    #[serde(default)]
    current: Option<Estimate<QuantityValue>>,
    #[serde(default)]
    forecast: Option<Estimate<QuantityValue>>,
}

impl<'de> Deserialize<'de> for QuantityState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = QuantityStateWire::deserialize(deserializer)?;
        Self::new(value.quantity, value.current, value.forecast).map_err(de::Error::custom)
    }
}

impl QuantityState {
    /// Creates native state after validating dimensions, support, and estimate metadata.
    pub fn new(
        quantity: QuantityDefinition,
        current: Option<Estimate<QuantityValue>>,
        forecast: Option<Estimate<QuantityValue>>,
    ) -> Result<Self, QuantityError> {
        let quantity = quantity.validated()?;
        quantity.fermi_target()?;
        Ok(Self {
            current: validate_estimate(current, &quantity)?,
            forecast: validate_estimate(forecast, &quantity)?,
            quantity,
        })
    }
}

fn validate_estimate(
    estimate: Option<Estimate<QuantityValue>>,
    quantity: &QuantityDefinition,
) -> Result<Option<Estimate<QuantityValue>>, QuantityError> {
    let Some(mut estimate) = estimate else {
        return Ok(None);
    };
    if !quantity.accepts(&estimate.distribution) {
        return Err(QuantityError::EstimateOutsideSupport);
    }
    if estimate
        .quantity
        .as_ref()
        .is_some_and(|persisted| persisted != quantity)
    {
        return Err(QuantityError::EstimateDefinitionMismatch);
    }
    estimate.quantity = Some(quantity.clone());
    Ok(Some(estimate))
}
