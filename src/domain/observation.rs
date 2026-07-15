use chrono::DateTime;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Measurement, Observation};

/// Unidentified measurement reading supplied when appending edge-owned evidence.
///
/// The owning [`Measurement`] allocates the ID and stores this value as an
/// immutable [`Observation`]. Unknown measurement error is represented by `None`,
/// never by silently assuming zero variance.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct NewObservation {
    /// Finite reading expressed in [`NewObservation::unit`].
    pub value: f64,
    /// Non-empty unit expected to match the source metric's definition.
    pub unit: String,
    /// RFC 3339 instant at which the measured event or period was observed.
    pub observed_at: String,
    /// Non-empty person, system, query, or citation which produced the reading.
    pub source: String,
    /// Known finite non-negative standard deviation of measurement error.
    pub measurement_standard_deviation: Option<f64>,
}

/// Validation and lifecycle failures for edge-owned observations.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ObservationError {
    /// The observed value is NaN or infinite and cannot enter statistical analysis.
    #[error("observation values must be finite")]
    NonFiniteValue,
    /// No unit was supplied for interpreting the numeric reading.
    #[error("an observation unit cannot be empty")]
    EmptyUnit,
    /// No evidence source was supplied for audit and calibration workflows.
    #[error("an observation source cannot be empty")]
    EmptySource,
    /// The observation timestamp is not an RFC 3339 instant.
    #[error("observation timestamps must use RFC 3339")]
    InvalidTimestamp,
    /// Measurement-error standard deviation is negative, NaN, or infinite.
    #[error("measurement standard deviation must be finite and non-negative")]
    InvalidStandardDeviation,
    /// The edge-local observation ID counter cannot allocate another value.
    #[error("the measurement has exhausted its observation identifier space")]
    IdentifierSpaceExhausted,
    /// No observation exists with the requested edge-local ID.
    #[error("observation {0} does not exist on this measurement")]
    NotFound(u64),
    /// A correction already supersedes this observation; corrections must form a chain.
    #[error("observation {0} has already been superseded")]
    AlreadySuperseded(u64),
}

impl Measurement {
    /// Validates and appends an immutable reading with the next edge-local ID.
    pub fn append(&mut self, input: NewObservation) -> Result<Observation, ObservationError> {
        validate(&input)?;
        let observation = Observation {
            id: next_id(&self.observations)?,
            revision: 0,
            value: input.value,
            unit: input.unit,
            observed_at: input.observed_at,
            source: input.source,
            measurement_standard_deviation: input.measurement_standard_deviation,
            supersedes: None,
        };
        self.observations.push(observation.clone());
        Ok(observation)
    }

    /// Appends an immutable correction while preserving the original evidence record.
    ///
    /// Unit, timestamp, source, and measurement error are copied from the predecessor;
    /// only the corrected numeric value changes. A record may be superseded once, but
    /// its correction may itself be corrected to form an auditable chain.
    pub fn correct(&mut self, id: u64, value: f64) -> Result<Observation, ObservationError> {
        if !value.is_finite() {
            return Err(ObservationError::NonFiniteValue);
        }
        if self
            .observations
            .iter()
            .any(|observation| observation.supersedes == Some(id))
        {
            return Err(ObservationError::AlreadySuperseded(id));
        }
        let original = self
            .observations
            .iter()
            .find(|observation| observation.id == id)
            .cloned()
            .ok_or(ObservationError::NotFound(id))?;
        let correction = Observation {
            id: next_id(&self.observations)?,
            revision: 0,
            value,
            supersedes: Some(id),
            ..original
        };
        self.observations.push(correction.clone());
        Ok(correction)
    }
}

fn validate(input: &NewObservation) -> Result<(), ObservationError> {
    if !input.value.is_finite() {
        return Err(ObservationError::NonFiniteValue);
    }
    if input.unit.trim().is_empty() {
        return Err(ObservationError::EmptyUnit);
    }
    if input.source.trim().is_empty() {
        return Err(ObservationError::EmptySource);
    }
    if DateTime::parse_from_rfc3339(&input.observed_at).is_err() {
        return Err(ObservationError::InvalidTimestamp);
    }
    if input
        .measurement_standard_deviation
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(ObservationError::InvalidStandardDeviation);
    }
    Ok(())
}

fn next_id(observations: &[Observation]) -> Result<u64, ObservationError> {
    match observations.iter().map(|observation| observation.id).max() {
        Some(id) => id
            .checked_add(1)
            .ok_or(ObservationError::IdentifierSpaceExhausted),
        None => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::{Measurement, NewObservation, ObservationError};
    use crate::domain::MeasurementPolarity;

    fn input(value: f64) -> NewObservation {
        NewObservation {
            value,
            unit: "ratio".to_owned(),
            observed_at: "2026-07-15T12:00:00Z".to_owned(),
            source: "deployment dashboard".to_owned(),
            measurement_standard_deviation: Some(0.02),
        }
    }

    #[test]
    fn appends_and_corrects_immutable_observations() {
        let mut measurement = Measurement {
            polarity: MeasurementPolarity::HigherIsBetter,
            observations: vec![],
        };
        let original = measurement.append(input(0.9)).unwrap();
        let correction = measurement.correct(original.id, 0.95).unwrap();
        assert_eq!(correction.supersedes, Some(original.id));
        assert_eq!(measurement.observations[0].value, 0.9);
        assert_eq!(measurement.observations[1].value, 0.95);
        assert_eq!(
            measurement.correct(original.id, 0.96),
            Err(ObservationError::AlreadySuperseded(original.id))
        );
    }

    #[test]
    fn rejects_invalid_statistical_inputs() {
        assert_eq!(
            Measurement {
                polarity: MeasurementPolarity::HigherIsBetter,
                observations: vec![],
            }
            .append(NewObservation {
                measurement_standard_deviation: Some(-0.1),
                ..input(f64::NAN)
            }),
            Err(ObservationError::NonFiniteValue)
        );
    }
}
