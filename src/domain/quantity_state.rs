use serde::{Deserialize, Deserializer, Serialize, de};

use super::{
    Estimate, EstimateSource, QuantityDefinition, QuantityError, QuantityValue, StateRelation,
    assess_squiggle_estimate,
};

/// Native-unit current and forecast state owned by a factor or outcome node.
///
/// Native state requires canonical unit terms and validates both estimates
/// against the same quantity support.
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
    /// Optional node equation replacing proportional composition for this state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation: Option<StateRelation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuantityStateWire {
    quantity: QuantityDefinition,
    #[serde(default)]
    current: Option<Estimate<QuantityValue>>,
    #[serde(default)]
    forecast: Option<Estimate<QuantityValue>>,
    #[serde(default)]
    relation: Option<StateRelation>,
}

impl<'de> Deserialize<'de> for QuantityState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = QuantityStateWire::deserialize(deserializer)?;
        Self::new(value.quantity, value.current, value.forecast)
            .map(|state| state.with_relation(value.relation))
            .map_err(de::Error::custom)
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
        quantity.estimate_target()?;
        Ok(Self {
            current: validate_estimate(current, &quantity)?,
            forecast: validate_estimate(forecast, &quantity)?,
            quantity,
            relation: None,
        })
    }

    /// Attaches or clears the node equation computing this state each period.
    ///
    /// Whether the equation type-checks depends on the parents the graph gives
    /// it, so that is verified when the owning project applies it.
    #[must_use]
    pub fn with_relation(mut self, relation: Option<StateRelation>) -> Self {
        self.relation = relation;
        self
    }

    pub(crate) fn with_quantity(self, quantity: QuantityDefinition) -> Result<Self, QuantityError> {
        let quantity = quantity.validated()?;
        let (_, unit) = quantity.estimate_target()?;
        Ok(Self {
            current: retarget_estimate(self.current, &quantity, &unit)?,
            forecast: retarget_estimate(self.forecast, &quantity, &unit)?,
            quantity,
            relation: self.relation,
        })
    }
}

fn retarget_estimate(
    estimate: Option<Estimate<QuantityValue>>,
    quantity: &QuantityDefinition,
    unit: &crate::domain::Unit,
) -> Result<Option<Estimate<QuantityValue>>, QuantityError> {
    let Some(mut estimate) = estimate else {
        return Ok(None);
    };
    let EstimateSource::Squiggle { definition } = estimate.source;
    let mut definition = *definition;
    definition.target_unit = unit.clone();
    let (definition, _, distribution) = assess_squiggle_estimate(definition, unit)?;
    if !quantity.accepts(&distribution) {
        return Err(QuantityError::EstimateOutsideSupport);
    }
    estimate.distribution = distribution;
    estimate.quantity = Some(quantity.clone());
    estimate.source = EstimateSource::Squiggle {
        definition: Box::new(definition),
    };
    Ok(Some(estimate))
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
