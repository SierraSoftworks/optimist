use std::{fmt, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{
    EntityId, EstimateUncertainty, EstimateUncertaintyError, QuantityDefinition,
    SquiggleEstimateDefinition, SquiggleEstimateError, Unit, assess_squiggle_estimate,
};

const MAX_EMPIRICAL_SAMPLES: usize = 4_096;

/// Identifies an estimate within its owning node or edge aggregate.
///
/// Estimate IDs are local because estimates are embedded values rather than graph
/// vertices. A complete reference also includes the owning project and aggregate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EstimateId(EntityId);

impl EstimateId {
    /// Constructs an aggregate-local estimate ID from its monotonic counter.
    ///
    /// ```
    /// use optimist::domain::EstimateId;
    /// assert_eq!(EstimateId::new(0).to_string(), "A");
    /// ```
    pub const fn new(value: u64) -> Self {
        Self(EntityId::new(value))
    }

    /// Returns the monotonic counter behind this aggregate-local ID.
    ///
    /// Owners which allocate several estimates at once, such as a temporal
    /// profile, use this to issue IDs above every value already in use.
    ///
    /// ```
    /// use optimist::domain::EstimateId;
    /// assert_eq!(EstimateId::new(3).value(), 3);
    /// ```
    pub const fn value(self) -> u64 {
        self.0.value()
    }
}

impl fmt::Display for EstimateId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum DistributionKind {
    Point {
        value: f64,
    },
    Normal {
        mean: f64,
        standard_deviation: f64,
    },
    LogNormal {
        location: f64,
        scale: f64,
    },
    Beta {
        alpha: f64,
        beta: f64,
    },
    ScaledBeta {
        alpha: f64,
        beta: f64,
        lower: f64,
        upper: f64,
    },
    Empirical {
        samples: Vec<f64>,
    },
}

/// Validation failures for primitive probability distributions.
///
/// Invalid distributions are rejected before persistence so analysis code can
/// assume finite parameters, valid support, and normalized family definitions.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum DistributionError {
    /// At least one supplied parameter is NaN or infinite.
    #[error("distribution parameters must be finite")]
    NonFinite,
    /// A Normal standard deviation or LogNormal log-scale is not positive.
    #[error("a standard deviation or scale must be greater than zero")]
    InvalidScale,
    /// A Beta shape parameter is not positive.
    #[error("beta shape parameters must be greater than zero")]
    InvalidShape,
    /// A scaled Beta's lower bound is not strictly below its upper bound.
    #[error("a scaled beta distribution requires lower < upper")]
    InvalidBounds,
    /// An empirical approximation must contain a bounded nonempty set of finite draws.
    #[error("an empirical distribution requires 1 to 4,096 finite samples")]
    InvalidSamples,
}

/// A validated primitive probability distribution used by an [`Estimate`].
///
/// Constructors encode Optimist's parameter conventions and reject invalid values.
/// The enum representation remains private so deserialization cannot bypass those
/// checks.
///
/// ```
/// use optimist::domain::Distribution;
///
/// let success_rate = Distribution::beta(8.0, 2.0)?;
/// assert!((success_rate.mean() - 0.8).abs() < f64::EPSILON);
/// # Ok::<(), optimist::domain::DistributionError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Distribution(pub(super) DistributionKind);

impl Distribution {
    /// Creates a deterministic distribution concentrated at `value`.
    ///
    /// Point masses represent measured constants or assumptions with no modelled
    /// uncertainty. They should not be used merely because uncertainty is unknown.
    pub fn point(value: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Point { value })
    }

    /// Creates a Normal distribution parameterized by mean and standard deviation.
    ///
    /// Use this for unbounded additive uncertainty. Bounded probabilities and
    /// non-negative quantities should use Beta or LogNormal families instead.
    pub fn normal(mean: f64, standard_deviation: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Normal {
            mean,
            standard_deviation,
        })
    }

    /// Creates a LogNormal distribution in log-space parameterization.
    ///
    /// If `X` is returned, then `ln(X) ~ Normal(location, scale²)`. This family is
    /// appropriate for positive multiplicative quantities such as costs and durations.
    pub fn log_normal(location: f64, scale: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::LogNormal { location, scale })
    }

    /// Creates a standard Beta distribution on the closed interval `[0, 1]`.
    ///
    /// Positive `alpha` and `beta` can represent success rates and normalized
    /// states, and support conjugate updating with Binomial observations.
    pub fn beta(alpha: f64, beta: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Beta { alpha, beta })
    }

    /// Creates a Beta distribution affinely transformed onto `[lower, upper]`.
    ///
    /// This is used for bounded signed influence estimates, commonly with bounds
    /// `-1` and `1`, while retaining the shape flexibility of a Beta distribution.
    pub fn scaled_beta(
        alpha: f64,
        beta: f64,
        lower: f64,
        upper: f64,
    ) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::ScaledBeta {
            alpha,
            beta,
            lower,
            upper,
        })
    }

    /// Creates an empirical distribution from a bounded set of retained draws.
    ///
    /// Empirical distributions preserve arbitrary Squiggle derivations for later
    /// scenario sampling. Samples retain their generated order for deterministic
    /// replay; statistics and quantiles sort temporary copies when necessary.
    pub fn empirical(samples: Vec<f64>) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Empirical { samples })
    }

    /// Returns a clamped quantile for this validated distribution.
    ///
    /// `probability` is clamped to the open interval representable by the
    /// implementation, avoiding infinite endpoint quantiles for unbounded families.
    /// Empirical quantiles use linear interpolation between ordered retained draws.
    pub fn quantile(&self, probability: f64) -> f64 {
        self.inverse_cdf(probability)
    }

    /// Borrows deterministic draws retained by an empirical distribution.
    ///
    /// Analytical families and point masses return `None`. Squiggle-authored
    /// distribution results retain empirical draws specifically so callers can
    /// perform finite prior-predictive checks without reevaluating source.
    pub fn retained_draws(&self) -> Option<&[f64]> {
        match &self.0 {
            DistributionKind::Empirical { samples } => Some(samples),
            _ => None,
        }
    }

    fn validated(kind: DistributionKind) -> Result<Self, DistributionError> {
        let parameters_are_finite = match kind {
            DistributionKind::Point { value } => value.is_finite(),
            DistributionKind::Normal {
                mean,
                standard_deviation,
            } => mean.is_finite() && standard_deviation.is_finite(),
            DistributionKind::LogNormal { location, scale } => {
                location.is_finite() && scale.is_finite()
            }
            DistributionKind::Beta { alpha, beta } => alpha.is_finite() && beta.is_finite(),
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => alpha.is_finite() && beta.is_finite() && lower.is_finite() && upper.is_finite(),
            DistributionKind::Empirical { ref samples } => {
                !samples.is_empty()
                    && samples.len() <= MAX_EMPIRICAL_SAMPLES
                    && samples.iter().all(|sample| sample.is_finite())
            }
        };
        if !parameters_are_finite {
            return Err(DistributionError::NonFinite);
        }

        match kind {
            DistributionKind::Normal {
                standard_deviation, ..
            } if standard_deviation <= 0.0 => Err(DistributionError::InvalidScale),
            DistributionKind::LogNormal { scale, .. } if scale <= 0.0 => {
                Err(DistributionError::InvalidScale)
            }
            DistributionKind::Beta { alpha, beta }
            | DistributionKind::ScaledBeta { alpha, beta, .. }
                if alpha <= 0.0 || beta <= 0.0 =>
            {
                Err(DistributionError::InvalidShape)
            }
            DistributionKind::ScaledBeta { lower, upper, .. } if lower >= upper => {
                Err(DistributionError::InvalidBounds)
            }
            DistributionKind::Empirical { ref samples }
                if samples.is_empty()
                    || samples.len() > MAX_EMPIRICAL_SAMPLES
                    || samples.iter().any(|sample| !sample.is_finite()) =>
            {
                Err(DistributionError::InvalidSamples)
            }
            _ => Ok(Self(kind)),
        }
    }

    pub(super) fn is_non_negative(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => value >= 0.0,
            DistributionKind::LogNormal { .. } | DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, .. } => lower >= 0.0,
            DistributionKind::Normal { .. } => false,
            DistributionKind::Empirical { ref samples } => {
                samples.iter().all(|sample| *sample >= 0.0)
            }
        }
    }

    pub(super) fn is_within(&self, lower: f64, upper: f64) -> bool {
        match self.0 {
            DistributionKind::Point { value } => (lower..=upper).contains(&value),
            DistributionKind::Beta { .. } => lower <= 0.0 && upper >= 1.0,
            DistributionKind::ScaledBeta {
                lower: actual_lower,
                upper: actual_upper,
                ..
            } => actual_lower >= lower && actual_upper <= upper,
            DistributionKind::Normal { .. } | DistributionKind::LogNormal { .. } => false,
            DistributionKind::Empirical { ref samples } => samples
                .iter()
                .all(|sample| (lower..=upper).contains(sample)),
        }
    }

    fn is_quantity_value(&self) -> bool {
        true
    }

    pub(super) fn is_probability(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => (0.0..=1.0).contains(&value),
            DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, upper, .. } => lower >= 0.0 && upper <= 1.0,
            DistributionKind::Empirical { ref samples } => {
                samples.iter().all(|sample| (0.0..=1.0).contains(sample))
            }
            _ => false,
        }
    }

    pub(super) fn is_signed_influence(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => (-1.0..=1.0).contains(&value),
            DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, upper, .. } => lower >= -1.0 && upper <= 1.0,
            DistributionKind::Empirical { ref samples } => {
                samples.iter().all(|sample| (-1.0..=1.0).contains(sample))
            }
            _ => false,
        }
    }
}

impl<'de> Deserialize<'de> for Distribution {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let kind = DistributionKind::deserialize(deserializer)?;
        Self::validated(kind).map_err(de::Error::custom)
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A sealed marker describing the physical or semantic support of an estimate.
///
/// Users select dimensions through marker types such as [`Money`] or
/// [`SignedInfluence`]. The trait is sealed so downstream crates cannot weaken
/// validation assumptions relied upon by statistical operations.
pub trait EstimateDimension: sealed::Sealed {
    /// Stable dimension name used in validation errors and serialized addresses.
    const NAME: &'static str;

    /// Reports whether a primitive distribution's complete support fits the dimension.
    fn accepts(distribution: &Distribution) -> bool;

    /// Returns quantity metadata intrinsically owned by this estimate dimension.
    fn quantity_definition() -> Option<QuantityDefinition> {
        None
    }

    /// Reports whether an owner may persist an explicit quantity definition.
    fn accepts_explicit_quantity() -> bool {
        false
    }
}

macro_rules! dimension {
    ($name:ident, $label:literal, $validator:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $name;

        impl sealed::Sealed for $name {}

        impl EstimateDimension for $name {
            const NAME: &'static str = $label;

            fn accepts(distribution: &Distribution) -> bool {
                distribution.$validator()
            }
        }
    };
}

dimension!(
    Money,
    "money",
    is_non_negative,
    "A non-negative monetary or cost quantity used when comparing intervention investment."
);
dimension!(
    Duration,
    "duration",
    is_non_negative,
    "A non-negative elapsed-time quantity used for costs, lags, and planning horizons."
);
dimension!(
    Probability,
    "probability",
    is_probability,
    "A probability whose complete support lies within `[0, 1]`, used for uncertain success or occurrence."
);
dimension!(
    SignedInfluence,
    "signed_influence",
    is_signed_influence,
    "A bounded causal effect on `[-1, 1]`, where sign gives direction and magnitude gives local strength."
);
/// A scalar value whose support and unit are supplied by its owning quantity definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuantityValue;

impl sealed::Sealed for QuantityValue {}

impl EstimateDimension for QuantityValue {
    const NAME: &'static str = "quantity_value";

    fn accepts(distribution: &Distribution) -> bool {
        distribution.is_quantity_value()
    }

    fn accepts_explicit_quantity() -> bool {
        true
    }
}

/// Errors returned when a primitive distribution cannot form a typed estimate.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateError {
    /// The primitive distribution itself contains invalid parameters.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
    /// The distribution has support outside the estimate dimension's legal range.
    #[error("distribution support is invalid for estimate dimension {0}")]
    InvalidSupport(&'static str),
    /// Persisted Squiggle source or deterministic controls are invalid.
    #[error(transparent)]
    Squiggle(#[from] SquiggleEstimateError),
    /// Descriptive uncertainty metadata exceeded its transport bound.
    #[error(transparent)]
    Uncertainty(#[from] EstimateUncertaintyError),
    /// Persisted intrinsic quantity metadata conflicts with the estimate dimension.
    #[error("quantity definition is invalid for estimate dimension {0}")]
    InvalidQuantityDefinition(&'static str),
}

/// Exclusive Squiggle source used to derive an estimate at runtime.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EstimateSource {
    /// A retained Squiggle calculation evaluated deterministically by the backend.
    Squiggle {
        /// Reviewable authored source and deterministic evaluation controls.
        definition: Box<SquiggleEstimateDefinition>,
    },
}

/// An uncertain, revisioned quantity embedded in a node or edge payload.
///
/// The marker `T` makes dimensional mistakes unrepresentable: for example, a
/// Normal distribution cannot be used as [`Estimate<Probability>`]. Provenance
/// records the evidence or elicitation context behind the prior.
///
/// ```
/// use optimist::domain::{Estimate, EstimateId, Probability, SquiggleEstimateDefinition, Unit};
///
/// let estimate = Estimate::<Probability>::from_squiggle(
///     EstimateId::new(0),
///     SquiggleEstimateDefinition {
///         source: "beta(8, 2)".to_owned(),
///         seed: 42,
///         sample_count: 256,
///         target_unit: Unit::dimensionless(),
///     },
///     &Unit::dimensionless(),
/// )?;
/// assert_eq!(estimate.revision, 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(bound = "")]
pub struct Estimate<T: EstimateDimension> {
    /// Stable ID used to address this estimate within its owning aggregate.
    pub id: EstimateId,
    /// Optimistic-concurrency revision incremented when the estimate changes.
    pub revision: u64,
    /// Runtime distribution derived from [`Self::source`]; never persisted.
    #[serde(skip)]
    pub distribution: Distribution,
    /// Explicit quantity metadata intrinsically owned by this estimate dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<QuantityDefinition>,
    /// Authoritative Squiggle source and deterministic evaluation controls.
    pub source: EstimateSource,
    /// Human-readable evidence, source, or elicitation records supporting the estimate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
    /// Distinct uncertainty sources retained without assuming independence.
    #[serde(default, skip_serializing_if = "EstimateUncertainty::is_empty")]
    pub uncertainty: EstimateUncertainty,
    #[serde(skip)]
    marker: PhantomData<T>,
}

impl<T: EstimateDimension> Estimate<T> {
    #[cfg(test)]
    pub(crate) fn new(id: EstimateId, distribution: Distribution) -> Result<Self, EstimateError> {
        let source = match &distribution.0 {
            DistributionKind::Point { value } => format!("pointMass({value})"),
            DistributionKind::Normal {
                mean,
                standard_deviation,
            } => format!("normal({mean}, {standard_deviation})"),
            DistributionKind::LogNormal { location, scale } => {
                format!("lognormal({location}, {scale})")
            }
            DistributionKind::Beta { alpha, beta } => format!("beta({alpha}, {beta})"),
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => format!("{lower} + ({upper} - {lower}) * beta({alpha}, {beta})"),
            DistributionKind::Empirical { .. } => {
                return Err(DistributionError::InvalidSamples.into());
            }
        };
        Self::from_squiggle(
            id,
            SquiggleEstimateDefinition {
                source,
                seed: 42,
                sample_count: 256,
                target_unit: Unit::dimensionless(),
            },
            &Unit::dimensionless(),
        )
    }

    pub(crate) fn from_evaluated_squiggle(
        id: EstimateId,
        distribution: Distribution,
        source: EstimateSource,
    ) -> Result<Self, EstimateError> {
        if !T::accepts(&distribution) {
            return Err(EstimateError::InvalidSupport(T::NAME));
        }

        Ok(Self {
            id,
            revision: 0,
            distribution,
            quantity: T::quantity_definition(),
            source,
            provenance: Vec::new(),
            uncertainty: EstimateUncertainty::default(),
            marker: PhantomData,
        })
    }

    /// Constructs a Squiggle-sourced estimate from deterministic backend evaluation.
    pub fn from_squiggle(
        id: EstimateId,
        definition: SquiggleEstimateDefinition,
        expected_unit: &Unit,
    ) -> Result<Self, EstimateError> {
        let (definition, _, distribution) = assess_squiggle_estimate(definition, expected_unit)?;
        let source = EstimateSource::Squiggle {
            definition: Box::new(definition),
        };
        Self::from_evaluated_squiggle(id, distribution, source)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEstimate {
    id: EstimateId,
    revision: u64,
    #[serde(default)]
    quantity: Option<QuantityDefinition>,
    source: EstimateSource,
    #[serde(default)]
    provenance: Vec<String>,
    #[serde(default)]
    uncertainty: EstimateUncertainty,
}

impl<'de, T: EstimateDimension> Deserialize<'de> for Estimate<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawEstimate::deserialize(deserializer)?;
        let mut estimate = match raw.source {
            EstimateSource::Squiggle { definition } => {
                let expected_unit = definition.target_unit.clone();
                Self::from_squiggle(raw.id, *definition, &expected_unit)
                    .map_err(de::Error::custom)?
            }
        };
        estimate.revision = raw.revision;
        let expected_quantity = T::quantity_definition();
        estimate.quantity = match (expected_quantity, raw.quantity) {
            (Some(expected), None) => Some(expected),
            (Some(expected), Some(persisted)) if expected == persisted => Some(expected),
            (None, persisted) if T::accepts_explicit_quantity() => persisted,
            (None, None) => None,
            _ => {
                return Err(de::Error::custom(EstimateError::InvalidQuantityDefinition(
                    T::NAME,
                )));
            }
        };
        estimate.provenance = raw.provenance;
        estimate.uncertainty = raw.uncertainty;
        Ok(estimate)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Distribution, DistributionError, Estimate, EstimateError, EstimateId, Money, Probability,
        SignedInfluence,
    };
    use crate::domain::{EstimateUncertainty, SquiggleEstimateDefinition, Unit};

    fn definition(source: &str, unit: Unit) -> SquiggleEstimateDefinition {
        SquiggleEstimateDefinition {
            source: source.to_owned(),
            seed: 42,
            sample_count: 256,
            target_unit: unit,
        }
    }

    #[test]
    fn rejects_invalid_distribution_parameters() {
        assert_eq!(
            Distribution::normal(0.0, 0.0),
            Err(DistributionError::InvalidScale)
        );
        assert_eq!(
            Distribution::beta(-1.0, 2.0),
            Err(DistributionError::InvalidShape)
        );
    }

    #[test]
    fn estimate_dimensions_reject_invalid_support() {
        assert_eq!(
            Estimate::<Probability>::from_squiggle(
                EstimateId::new(1),
                definition("normal(0.5, 10)", Unit::dimensionless()),
                &Unit::dimensionless(),
            ),
            Err(EstimateError::InvalidSupport("probability"))
        );
        assert!(
            Estimate::<Money>::from_squiggle(
                EstimateId::new(2),
                definition("lognormal(1, 0.2)", Unit::dimensionless()),
                &Unit::dimensionless(),
            )
            .is_ok()
        );
        assert!(
            Estimate::<SignedInfluence>::from_squiggle(
                EstimateId::new(3),
                definition("2 * beta(2, 2) - 1", Unit::dimensionless()),
                &Unit::dimensionless(),
            )
            .is_ok()
        );
    }

    #[test]
    fn missing_source_cannot_enter_through_json() {
        let json = r#"{
            "id":"B",
            "revision":0,
            "distribution":{"type":"normal","mean":0.5,"standard_deviation":0.1}
        }"#;
        assert!(serde_json::from_str::<Estimate<Probability>>(json).is_err());
    }

    #[test]
    fn uncertainty_sources_round_trip_without_changing_the_distribution() {
        let mut estimate = Estimate::<Probability>::from_squiggle(
            EstimateId::new(0),
            definition("beta(2, 3)", Unit::dimensionless()),
            &Unit::dimensionless(),
        )
        .unwrap();
        estimate.uncertainty = EstimateUncertainty::new(
            "  Limited calibration data  ",
            "Week-to-week demand variation",
            "Sampling error",
        )
        .unwrap();

        let restored: Estimate<Probability> =
            serde_json::from_str(&serde_json::to_string(&estimate).unwrap()).unwrap();

        assert_eq!(restored.distribution, estimate.distribution);
        assert_eq!(restored.uncertainty, estimate.uncertainty);
        assert_eq!(restored.uncertainty.epistemic, "Limited calibration data");
    }

    #[test]
    fn squiggle_sources_round_trip_without_serializing_derived_results() {
        let estimate = Estimate::<Probability>::from_squiggle(
            EstimateId::new(0),
            SquiggleEstimateDefinition {
                source: "beta(2, 2)".to_owned(),
                seed: 42,
                sample_count: 2_048,
                target_unit: Unit::dimensionless(),
            },
            &Unit::dimensionless(),
        )
        .unwrap();
        let value = serde_json::to_value(&estimate).unwrap();
        assert!(value.get("distribution").is_none());
        assert!(value["source"].get("assessment").is_none());

        let restored = serde_json::from_value::<Estimate<Probability>>(value).unwrap();
        assert_eq!(restored, estimate);
    }
}
