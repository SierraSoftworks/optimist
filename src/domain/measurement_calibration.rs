use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Explicit mapping from readings in a metric's unit to normalized subject state.
///
/// Calibration is attached to one `metric -> subject` measurement relationship because
/// the same metric can have different meaningful thresholds for different factors or
/// outcomes. Mappings clamp outside their anchors and always return a state on `[0, 1]`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MeasurementCalibration {
    /// Affine mapping where two readings identify normalized states zero and one.
    Linear {
        /// Metric reading interpreted as normalized state zero.
        state_zero: f64,
        /// Metric reading interpreted as normalized state one.
        state_one: f64,
    },
    /// Trapezoidal mapping for a metric whose ideal readings lie inside a range.
    TargetRange {
        /// Reading at or below which normalized state is zero.
        outer_lower: f64,
        /// Lower edge of the ideal interval where normalized state reaches one.
        ideal_lower: f64,
        /// Upper edge of the ideal interval where normalized state remains one.
        ideal_upper: f64,
        /// Reading at or above which normalized state returns to zero.
        outer_upper: f64,
    },
}

/// Invalid anchors which do not define an unambiguous metric-to-state mapping.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum MeasurementCalibrationError {
    /// Every calibration anchor must be finite.
    #[error("measurement calibration anchors must be finite")]
    NonFinite,
    /// Linear state-zero and state-one readings cannot be equal.
    #[error("linear measurement calibration requires distinct state-zero and state-one readings")]
    EqualLinearAnchors,
    /// Target-range anchors must define two nonempty ramps around an ordered ideal range.
    #[error(
        "target-range anchors must satisfy outer_lower < ideal_lower <= ideal_upper < outer_upper"
    )]
    InvalidTargetRange,
    /// A reading being translated into state must be finite.
    #[error("a metric reading must be finite before it can be translated to state")]
    NonFiniteReading,
    /// The calibration shape or direction disagrees with the relationship polarity.
    #[error("measurement calibration does not match {0:?} polarity")]
    PolarityMismatch(super::MeasurementPolarity),
}

impl MeasurementCalibration {
    /// Validates finite, ordered anchors and returns the calibration unchanged.
    pub fn validated(self) -> Result<Self, MeasurementCalibrationError> {
        match &self {
            Self::Linear {
                state_zero,
                state_one,
            } if !state_zero.is_finite() || !state_one.is_finite() => {
                Err(MeasurementCalibrationError::NonFinite)
            }
            Self::Linear {
                state_zero,
                state_one,
            } if state_zero == state_one => Err(MeasurementCalibrationError::EqualLinearAnchors),
            Self::TargetRange {
                outer_lower,
                ideal_lower,
                ideal_upper,
                outer_upper,
            } if [*outer_lower, *ideal_lower, *ideal_upper, *outer_upper]
                .iter()
                .any(|value| !value.is_finite()) =>
            {
                Err(MeasurementCalibrationError::NonFinite)
            }
            Self::TargetRange {
                outer_lower,
                ideal_lower,
                ideal_upper,
                outer_upper,
            } if !(outer_lower < ideal_lower
                && ideal_lower <= ideal_upper
                && ideal_upper < outer_upper) =>
            {
                Err(MeasurementCalibrationError::InvalidTargetRange)
            }
            _ => Ok(self),
        }
    }

    /// Translates one finite metric reading into a clamped normalized state.
    ///
    /// For linear anchors $(x_0,x_1)$, state is
    /// $\operatorname{clamp}((x-x_0)/(x_1-x_0),0,1)$, so reversed anchors naturally
    /// model lower-is-better metrics. Target ranges use linear ramps from each outer
    /// zero anchor to the nearest ideal one anchor and a state-one plateau in between.
    pub fn state(&self, reading: f64) -> Result<f64, MeasurementCalibrationError> {
        if !reading.is_finite() {
            return Err(MeasurementCalibrationError::NonFiniteReading);
        }
        let state = match self {
            Self::Linear {
                state_zero,
                state_one,
            } => (reading - state_zero) / (state_one - state_zero),
            Self::TargetRange {
                outer_lower,
                ideal_lower,
                ideal_upper,
                outer_upper,
            } if reading < *ideal_lower => (reading - outer_lower) / (ideal_lower - outer_lower),
            Self::TargetRange {
                ideal_upper,
                outer_upper,
                ..
            } if reading > *ideal_upper => (outer_upper - reading) / (outer_upper - ideal_upper),
            Self::TargetRange { .. } => 1.0,
        };
        Ok(state.clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_anchors_map_endpoints_midpoints_reversal_and_clamping() {
        let higher = MeasurementCalibration::Linear {
            state_zero: 10.0,
            state_one: 30.0,
        }
        .validated()
        .unwrap();
        assert_eq!(higher.state(10.0).unwrap(), 0.0);
        assert_eq!(higher.state(20.0).unwrap(), 0.5);
        assert_eq!(higher.state(40.0).unwrap(), 1.0);

        let lower = MeasurementCalibration::Linear {
            state_zero: 30.0,
            state_one: 10.0,
        }
        .validated()
        .unwrap();
        assert_eq!(lower.state(20.0).unwrap(), 0.5);
        assert_eq!(lower.state(5.0).unwrap(), 1.0);
    }

    #[test]
    fn target_range_maps_ramps_plateau_and_outer_values() {
        let calibration = MeasurementCalibration::TargetRange {
            outer_lower: 50.0,
            ideal_lower: 80.0,
            ideal_upper: 120.0,
            outer_upper: 150.0,
        }
        .validated()
        .unwrap();
        assert_eq!(calibration.state(40.0).unwrap(), 0.0);
        assert_eq!(calibration.state(65.0).unwrap(), 0.5);
        assert_eq!(calibration.state(100.0).unwrap(), 1.0);
        assert_eq!(calibration.state(135.0).unwrap(), 0.5);
        assert_eq!(calibration.state(160.0).unwrap(), 0.0);
    }

    #[test]
    fn rejects_ambiguous_or_non_finite_anchors() {
        assert_eq!(
            MeasurementCalibration::Linear {
                state_zero: 1.0,
                state_one: 1.0,
            }
            .validated(),
            Err(MeasurementCalibrationError::EqualLinearAnchors)
        );
        assert!(matches!(
            MeasurementCalibration::TargetRange {
                outer_lower: 0.0,
                ideal_lower: 2.0,
                ideal_upper: 1.0,
                outer_upper: 3.0,
            }
            .validated(),
            Err(MeasurementCalibrationError::InvalidTargetRange)
        ));
    }
}
