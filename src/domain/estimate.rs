use std::{fmt, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::EntityId;

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
    /// appropriate for positive Fermi factors such as costs and durations.
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
            _ => Ok(Self(kind)),
        }
    }

    fn is_non_negative(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => value >= 0.0,
            DistributionKind::LogNormal { .. } | DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, .. } => lower >= 0.0,
            DistributionKind::Normal { .. } => false,
        }
    }

    fn is_probability(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => (0.0..=1.0).contains(&value),
            DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, upper, .. } => lower >= 0.0 && upper <= 1.0,
            _ => false,
        }
    }

    fn is_signed_influence(&self) -> bool {
        match self.0 {
            DistributionKind::Point { value } => (-1.0..=1.0).contains(&value),
            DistributionKind::Beta { .. } => true,
            DistributionKind::ScaledBeta { lower, upper, .. } => lower >= -1.0 && upper <= 1.0,
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
    NormalizedState,
    "normalized_state",
    is_probability,
    "A normalized factor or outcome state on `[0, 1]`, enabling relative causal models without pretending to have calibrated units."
);
dimension!(
    SignedInfluence,
    "signed_influence",
    is_signed_influence,
    "A bounded causal effect on `[-1, 1]`, where sign gives direction and magnitude gives local strength."
);

/// Errors returned when a primitive distribution cannot form a typed estimate.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateError {
    /// The primitive distribution itself contains invalid parameters.
    #[error(transparent)]
    Distribution(#[from] DistributionError),
    /// The distribution has support outside the estimate dimension's legal range.
    #[error("distribution support is invalid for estimate dimension {0}")]
    InvalidSupport(&'static str),
}

/// An uncertain, revisioned quantity embedded in a node or edge payload.
///
/// The marker `T` makes dimensional mistakes unrepresentable: for example, a
/// Normal distribution cannot be used as [`Estimate<Probability>`]. Provenance
/// records the evidence or elicitation context behind the prior.
///
/// ```
/// use optimist::domain::{Distribution, Estimate, EstimateId, Probability};
///
/// let estimate = Estimate::<Probability>::new(
///     EstimateId::new(0),
///     Distribution::beta(8.0, 2.0)?,
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
    /// Validated prior distribution whose support is accepted by `T`.
    pub distribution: Distribution,
    /// Human-readable evidence, source, or elicitation records supporting the estimate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
    #[serde(skip)]
    marker: PhantomData<T>,
}

impl<T: EstimateDimension> Estimate<T> {
    /// Constructs an estimate after checking the distribution against `T`.
    ///
    /// Use this constructor instead of struct literals so invalid dimensional
    /// combinations are rejected before they reach storage or analysis.
    pub fn new(id: EstimateId, distribution: Distribution) -> Result<Self, EstimateError> {
        if !T::accepts(&distribution) {
            return Err(EstimateError::InvalidSupport(T::NAME));
        }

        Ok(Self {
            id,
            revision: 0,
            distribution,
            provenance: Vec::new(),
            marker: PhantomData,
        })
    }
}

#[derive(Deserialize)]
struct RawEstimate {
    id: EstimateId,
    revision: u64,
    distribution: Distribution,
    #[serde(default)]
    provenance: Vec<String>,
}

impl<'de, T: EstimateDimension> Deserialize<'de> for Estimate<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawEstimate::deserialize(deserializer)?;
        let mut estimate = Self::new(raw.id, raw.distribution).map_err(de::Error::custom)?;
        estimate.revision = raw.revision;
        estimate.provenance = raw.provenance;
        Ok(estimate)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Distribution, DistributionError, Estimate, EstimateError, EstimateId, Money, Probability,
        SignedInfluence,
    };

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
        let normal = Distribution::normal(0.5, 0.1).expect("valid normal");
        assert_eq!(
            Estimate::<Probability>::new(EstimateId::new(1), normal),
            Err(EstimateError::InvalidSupport("probability"))
        );
        assert!(
            Estimate::<Money>::new(
                EstimateId::new(2),
                Distribution::log_normal(1.0, 0.2).expect("valid log-normal")
            )
            .is_ok()
        );
        assert!(
            Estimate::<SignedInfluence>::new(
                EstimateId::new(3),
                Distribution::scaled_beta(2.0, 2.0, -1.0, 1.0).expect("valid scaled beta")
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_support_cannot_enter_through_json() {
        let json = r#"{
            "id":"B",
            "revision":0,
            "distribution":{"type":"normal","mean":0.5,"standard_deviation":0.1}
        }"#;
        assert!(serde_json::from_str::<Estimate<Probability>>(json).is_err());
    }
}
