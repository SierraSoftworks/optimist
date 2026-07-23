use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

const MAX_UNCERTAINTY_CONTEXT_BYTES: usize = 4_096;

/// Describes distinct contributors to an estimate's total uncertainty.
///
/// These fields retain modelling assumptions and do not assign numeric shares or
/// imply that the components are independent. The estimate's distribution remains
/// the authoritative representation of total uncertainty.
///
/// ```
/// use optimist::domain::EstimateUncertainty;
///
/// let uncertainty = EstimateUncertainty::new(
///     "Limited evidence about the adoption rate",
///     "Week-to-week variation in user behaviour",
///     "Sampling error in the analytics pipeline",
/// )?;
/// assert!(uncertainty.epistemic.contains("evidence"));
/// # Ok::<(), optimist::domain::EstimateUncertaintyError>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EstimateUncertainty {
    /// Reducible uncertainty from limited knowledge, evidence, or model structure.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub epistemic: String,
    /// Variation between future realizations of the underlying process.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub process: String,
    /// Error introduced while observing, sampling, or resolving the quantity.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub measurement: String,
}

#[derive(Deserialize)]
struct EstimateUncertaintyWire {
    #[serde(default)]
    epistemic: String,
    #[serde(default)]
    process: String,
    #[serde(default)]
    measurement: String,
}

impl<'de> Deserialize<'de> for EstimateUncertainty {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = EstimateUncertaintyWire::deserialize(deserializer)?;
        Self {
            epistemic: value.epistemic,
            process: value.process,
            measurement: value.measurement,
        }
        .validated()
        .map_err(de::Error::custom)
    }
}

impl EstimateUncertainty {
    /// Creates a validated descriptive uncertainty budget.
    pub fn new(
        epistemic: impl Into<String>,
        process: impl Into<String>,
        measurement: impl Into<String>,
    ) -> Result<Self, EstimateUncertaintyError> {
        Self {
            epistemic: epistemic.into(),
            process: process.into(),
            measurement: measurement.into(),
        }
        .validated()
    }

    pub(crate) fn validated(mut self) -> Result<Self, EstimateUncertaintyError> {
        self.epistemic = self.epistemic.trim().to_owned();
        self.process = self.process.trim().to_owned();
        self.measurement = self.measurement.trim().to_owned();
        for (name, value) in [
            ("epistemic", &self.epistemic),
            ("process", &self.process),
            ("measurement", &self.measurement),
        ] {
            if value.len() > MAX_UNCERTAINTY_CONTEXT_BYTES {
                return Err(EstimateUncertaintyError::ContextTooLarge(name));
            }
        }
        Ok(self)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.epistemic.is_empty() && self.process.is_empty() && self.measurement.is_empty()
    }
}

/// Invalid descriptive uncertainty metadata.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateUncertaintyError {
    /// One uncertainty category exceeded its bounded transport size.
    #[error("{0} uncertainty exceeds its maximum length")]
    ContextTooLarge(&'static str),
}
