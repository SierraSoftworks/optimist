use std::{fmt, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{
    EntityId, EstimateUncertainty, EstimateUncertaintyError, FermiAssessment,
    FermiEstimateDefinition, FermiEstimateError, QuantityDefinition, SquiggleEstimateAssessment,
    SquiggleEstimateDefinition, SquiggleEstimateError, Unit, assess_squiggle_estimate,
};

const MAX_EMPIRICAL_SAMPLES: usize = 4_096;
const SQUIGGLE_INTEGRITY_ULPS: f64 = 16.0;

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

    /// Creates an empirical distribution from a bounded set of retained draws.
    ///
    /// Empirical distributions preserve arbitrary Squiggle derivations for later
    /// scenario sampling. Samples retain their generated order for deterministic
    /// replay; statistics and quantiles sort temporary copies when necessary.
    pub fn empirical(samples: Vec<f64>) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Empirical { samples })
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

    fn is_probability(&self) -> bool {
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

    fn is_signed_influence(&self) -> bool {
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
/// A legacy standardized factor or outcome state on `[0, 1]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedState;

impl sealed::Sealed for NormalizedState {}

impl EstimateDimension for NormalizedState {
    const NAME: &'static str = "normalized_state";

    fn accepts(distribution: &Distribution) -> bool {
        distribution.is_probability()
    }

    fn quantity_definition() -> Option<QuantityDefinition> {
        Some(QuantityDefinition::legacy_standardized_state())
    }
}
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
    /// Persisted Fermi source metadata or assessment is inconsistent.
    #[error(transparent)]
    Fermi(#[from] FermiEstimateError),
    /// Persisted Squiggle source or assessment is invalid.
    #[error(transparent)]
    Squiggle(#[from] SquiggleEstimateError),
    /// Descriptive uncertainty metadata exceeded its transport bound.
    #[error(transparent)]
    Uncertainty(#[from] EstimateUncertaintyError),
    /// Persisted intrinsic quantity metadata conflicts with the estimate dimension.
    #[error("quantity definition is invalid for estimate dimension {0}")]
    InvalidQuantityDefinition(&'static str),
}

/// Exclusive source used to produce an estimate's effective distribution.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EstimateSource {
    /// The effective distribution was authored directly.
    #[default]
    Distribution,
    /// A retained equation was assessed into the effective distribution.
    Fermi {
        /// Reviewable equation, variables, canonical formula, and sampling controls.
        definition: Box<FermiEstimateDefinition>,
        /// Server-generated result and diagnostics retained with the estimate revision.
        assessment: Box<FermiAssessment>,
    },
    /// A retained Squiggle calculation evaluated by the Rust backend.
    Squiggle {
        /// Reviewable authored source and deterministic evaluation controls.
        definition: Box<SquiggleEstimateDefinition>,
        /// Server-generated family, moments, quantiles, and sampling metadata.
        assessment: Box<SquiggleEstimateAssessment>,
    },
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
    /// Explicit quantity metadata intrinsically owned by this estimate dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<QuantityDefinition>,
    /// Active authoring source; Fermi sources supersede direct distribution editing.
    #[serde(default)]
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
            quantity: T::quantity_definition(),
            source: EstimateSource::Distribution,
            provenance: Vec::new(),
            uncertainty: EstimateUncertainty::default(),
            marker: PhantomData,
        })
    }

    /// Constructs a Fermi-sourced estimate from one validated server assessment.
    pub fn from_fermi(
        id: EstimateId,
        definition: FermiEstimateDefinition,
        assessment: FermiAssessment,
    ) -> Result<Self, EstimateError> {
        let definition = definition.validated()?;
        let distribution = assessment
            .recommended_distribution()
            .cloned()
            .ok_or(FermiEstimateError::UnavailableRecommendation)?;
        let mut estimate = Self::new(id, distribution)?;
        estimate.source = EstimateSource::Fermi {
            definition: Box::new(definition),
            assessment: Box::new(assessment),
        };
        Ok(estimate)
    }

    /// Constructs a Squiggle-sourced estimate from deterministic backend evaluation.
    pub fn from_squiggle(
        id: EstimateId,
        definition: SquiggleEstimateDefinition,
        expected_unit: &Unit,
    ) -> Result<Self, EstimateError> {
        let (definition, assessment, distribution) =
            assess_squiggle_estimate(definition, expected_unit)?;
        let mut estimate = Self::new(id, distribution)?;
        estimate.source = EstimateSource::Squiggle {
            definition: Box::new(definition),
            assessment: Box::new(assessment),
        };
        Ok(estimate)
    }
}

#[derive(Deserialize)]
struct RawEstimate {
    id: EstimateId,
    revision: u64,
    distribution: Distribution,
    #[serde(default)]
    quantity: Option<QuantityDefinition>,
    #[serde(default)]
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
            EstimateSource::Distribution => {
                Self::new(raw.id, raw.distribution).map_err(de::Error::custom)?
            }
            EstimateSource::Fermi {
                definition,
                assessment,
            } => {
                let estimate = Self::from_fermi(raw.id, *definition, *assessment)
                    .map_err(de::Error::custom)?;
                if estimate.distribution != raw.distribution {
                    return Err(de::Error::custom(FermiEstimateError::ResultMismatch));
                }
                estimate
            }
            EstimateSource::Squiggle {
                definition,
                assessment,
            } => {
                let expected_unit = definition.target_unit.clone();
                let estimate = Self::from_squiggle(raw.id, *definition, &expected_unit)
                    .map_err(de::Error::custom)?;
                let EstimateSource::Squiggle {
                    assessment: evaluated,
                    ..
                } = &estimate.source
                else {
                    unreachable!()
                };
                if !squiggle_result_matches(
                    &estimate.distribution,
                    &raw.distribution,
                    evaluated,
                    &assessment,
                ) {
                    return Err(de::Error::custom(SquiggleEstimateError::ResultMismatch));
                }
                estimate
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

fn squiggle_result_matches(
    evaluated_distribution: &Distribution,
    persisted_distribution: &Distribution,
    evaluated_assessment: &SquiggleEstimateAssessment,
    persisted_assessment: &SquiggleEstimateAssessment,
) -> bool {
    evaluated_assessment.family == persisted_assessment.family
        && evaluated_assessment.seed == persisted_assessment.seed
        && evaluated_assessment.sample_count == persisted_assessment.sample_count
        && optional_float_matches(evaluated_assessment.mean, persisted_assessment.mean)
        && optional_float_matches(evaluated_assessment.variance, persisted_assessment.variance)
        && float_matches(evaluated_assessment.p05, persisted_assessment.p05)
        && float_matches(evaluated_assessment.p50, persisted_assessment.p50)
        && float_matches(evaluated_assessment.p95, persisted_assessment.p95)
        && distribution_result_matches(evaluated_distribution, persisted_distribution)
}

fn distribution_result_matches(evaluated: &Distribution, persisted: &Distribution) -> bool {
    match (&evaluated.0, &persisted.0) {
        (
            DistributionKind::Point { value: evaluated },
            DistributionKind::Point { value: persisted },
        ) => float_matches(*evaluated, *persisted),
        (
            DistributionKind::Empirical { samples: evaluated },
            DistributionKind::Empirical { samples: persisted },
        ) => {
            evaluated.len() == persisted.len()
                && evaluated
                    .iter()
                    .zip(persisted)
                    .all(|(evaluated, persisted)| float_matches(*evaluated, *persisted))
        }
        _ => false,
    }
}

fn optional_float_matches(evaluated: Option<f64>, persisted: Option<f64>) -> bool {
    match (evaluated, persisted) {
        (Some(evaluated), Some(persisted)) => float_matches(evaluated, persisted),
        (None, None) => true,
        _ => false,
    }
}

fn float_matches(evaluated: f64, persisted: f64) -> bool {
    // JSON and graph-store round trips may move a finite result by a few ULPs.
    // The relative bound is τ = 16 ε max(1, |x|, |y|); larger drift is corruption.
    let scale = evaluated.abs().max(persisted.abs()).max(1.0);
    (evaluated - persisted).abs() <= SQUIGGLE_INTEGRITY_ULPS * f64::EPSILON * scale
}

#[cfg(test)]
mod tests {
    use super::{
        Distribution, DistributionError, Estimate, EstimateError, EstimateId, EstimateSource,
        Money, NormalizedState, Probability, SignedInfluence,
    };
    use crate::domain::{
        EstimateUncertainty, FermiEstimateDefinition, FermiEstimateSupport,
        FermiExpressionLanguage, FermiVariable, FermiVariableUncertainty, Formula,
        MonteCarloConfig, ProjectId, SquiggleEstimateDefinition, Unit, assess_fermi,
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

    #[test]
    fn legacy_estimates_default_to_distribution_sources() {
        let json = r#"{
            "id":"A",
            "revision":0,
            "distribution":{"type":"beta","alpha":2.0,"beta":3.0}
        }"#;
        let estimate = serde_json::from_str::<Estimate<Probability>>(json).unwrap();
        assert_eq!(estimate.source, EstimateSource::Distribution);
        assert_eq!(estimate.uncertainty, EstimateUncertainty::default());
        assert!(
            !serde_json::to_string(&estimate)
                .unwrap()
                .contains("uncertainty")
        );
    }

    #[test]
    fn legacy_normalized_states_gain_explicit_standardized_quantity_metadata() {
        let json = r#"{
            "id":"A",
            "revision":0,
            "distribution":{"type":"beta","alpha":2.0,"beta":3.0}
        }"#;
        let estimate = serde_json::from_str::<Estimate<NormalizedState>>(json).unwrap();
        let serialized = serde_json::to_value(&estimate).unwrap();

        assert_eq!(estimate.distribution, Distribution::beta(2.0, 3.0).unwrap());
        assert_eq!(serialized["quantity"]["unit"], "standardized_state");
        assert_eq!(serialized["quantity"]["support"]["lower"], 0.0);
        assert_eq!(serialized["quantity"]["support"]["upper"], 1.0);

        let mut conflicting = serialized;
        conflicting["quantity"]["unit"] = serde_json::json!("probability");
        assert!(serde_json::from_value::<Estimate<NormalizedState>>(conflicting).is_err());
    }

    #[test]
    fn uncertainty_sources_round_trip_without_changing_the_distribution() {
        let mut estimate =
            Estimate::<Probability>::new(EstimateId::new(0), Distribution::beta(2.0, 3.0).unwrap())
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
    fn fermi_sources_round_trip_and_reject_tampered_results() {
        let formula = Formula::Literal {
            distribution: Distribution::point(0.5).unwrap(),
            unit: Unit::dimensionless(),
        };
        let assessment = assess_fermi(
            &ProjectId::new("A").unwrap(),
            formula.clone(),
            FermiEstimateSupport::Probability,
            Unit::dimensionless(),
            MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
        )
        .unwrap();
        let definition = FermiEstimateDefinition {
            language: FermiExpressionLanguage::OptimistSquiggleV1,
            equation: "confidence".to_owned(),
            variables: vec![FermiVariable {
                name: "confidence".to_owned(),
                estimate: 0.5,
                unit: String::new(),
                uncertainty: FermiVariableUncertainty::ThreePoint {
                    low: 0.5,
                    high: 0.5,
                },
            }],
            formula,
            monte_carlo: MonteCarloConfig::new(42, 100, 1_000, 0.01, 0.01).unwrap(),
        };
        let estimate =
            Estimate::<Probability>::from_fermi(EstimateId::new(0), definition, assessment)
                .unwrap();
        let value = serde_json::to_value(&estimate).unwrap();
        assert_eq!(
            serde_json::from_value::<Estimate<Probability>>(value.clone()).unwrap(),
            estimate
        );

        let mut tampered = value;
        tampered["distribution"] = serde_json::json!({
            "type": "point",
            "value": 0.75
        });
        assert!(serde_json::from_value::<Estimate<Probability>>(tampered).is_err());
    }

    #[test]
    fn squiggle_sources_round_trip_and_reject_tampered_effective_samples() {
        let estimate = Estimate::<NormalizedState>::from_squiggle(
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
        assert_eq!(
            serde_json::from_value::<Estimate<NormalizedState>>(value.clone()).unwrap(),
            estimate
        );
        let mut rounded = value.clone();
        let sample = rounded["distribution"]["samples"][3].as_f64().unwrap();
        rounded["distribution"]["samples"][3] =
            serde_json::json!(f64::from_bits(sample.to_bits() + 1));
        assert!(serde_json::from_value::<Estimate<NormalizedState>>(rounded).is_ok());
        let mut tampered = value;
        tampered["distribution"]["samples"][0] = serde_json::json!(2.0);
        assert!(serde_json::from_value::<Estimate<NormalizedState>>(tampered).is_err());
    }
}
