use std::{fmt, marker::PhantomData};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::EntityId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EstimateId(EntityId);

impl EstimateId {
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
enum DistributionKind {
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

#[derive(Clone, Debug, Error, PartialEq)]
pub enum DistributionError {
    #[error("distribution parameters must be finite")]
    NonFinite,
    #[error("a standard deviation or scale must be greater than zero")]
    InvalidScale,
    #[error("beta shape parameters must be greater than zero")]
    InvalidShape,
    #[error("a scaled beta distribution requires lower < upper")]
    InvalidBounds,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Distribution(DistributionKind);

impl Distribution {
    pub fn point(value: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Point { value })
    }

    pub fn normal(mean: f64, standard_deviation: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Normal {
            mean,
            standard_deviation,
        })
    }

    pub fn log_normal(location: f64, scale: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::LogNormal { location, scale })
    }

    pub fn beta(alpha: f64, beta: f64) -> Result<Self, DistributionError> {
        Self::validated(DistributionKind::Beta { alpha, beta })
    }

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

    pub fn mean(&self) -> f64 {
        match self.0 {
            DistributionKind::Point { value } => value,
            DistributionKind::Normal { mean, .. } => mean,
            DistributionKind::LogNormal { location, scale } => {
                (location + scale.powi(2) / 2.0).exp()
            }
            DistributionKind::Beta { alpha, beta } => alpha / (alpha + beta),
            DistributionKind::ScaledBeta {
                alpha,
                beta,
                lower,
                upper,
            } => lower + (upper - lower) * alpha / (alpha + beta),
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

pub trait EstimateDimension: sealed::Sealed {
    const NAME: &'static str;

    fn accepts(distribution: &Distribution) -> bool;
}

macro_rules! dimension {
    ($name:ident, $label:literal, $validator:ident) => {
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

dimension!(Money, "money", is_non_negative);
dimension!(Duration, "duration", is_non_negative);
dimension!(Probability, "probability", is_probability);
dimension!(NormalizedState, "normalized_state", is_probability);
dimension!(SignedInfluence, "signed_influence", is_signed_influence);

#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateError {
    #[error(transparent)]
    Distribution(#[from] DistributionError),
    #[error("distribution support is invalid for estimate dimension {0}")]
    InvalidSupport(&'static str),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(bound = "")]
pub struct Estimate<T: EstimateDimension> {
    pub id: EstimateId,
    pub revision: u64,
    pub distribution: Distribution,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<String>,
    #[serde(skip)]
    marker: PhantomData<T>,
}

impl<T: EstimateDimension> Estimate<T> {
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
