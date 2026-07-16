use crate::domain::{EdgePayload, Measurement, MeasurementPolarity, Requirement};

use super::edge::{EdgeType, Polarity, RequirementMode};

pub(super) fn build(
    kind: EdgeType,
    requirement: Option<RequirementMode>,
    threshold: Option<f64>,
    polarity: Option<Polarity>,
) -> Result<EdgePayload, human_errors::Error> {
    match kind {
        EdgeType::Requires if polarity.is_none() => {
            let mode = requirement
                .ok_or_else(|| invalid("Requires edges need `--requirement hard|soft`."))?;
            if threshold.is_some_and(|value| !(0.0..=1.0).contains(&value)) {
                return Err(invalid("Requirement thresholds must be between 0 and 1."));
            }
            Ok(EdgePayload::Requires(Requirement {
                hard: matches!(mode, RequirementMode::Hard),
                satisfaction_threshold: threshold,
            }))
        }
        EdgeType::Measures if requirement.is_none() && threshold.is_none() => {
            let polarity = polarity.ok_or_else(|| invalid("Measures edges need `--polarity`."))?;
            Ok(EdgePayload::Measures(Measurement {
                polarity: polarity.into(),
                calibration: None,
                observations: vec![],
            }))
        }
        EdgeType::PartOf if no_options(requirement, threshold, polarity) => Ok(EdgePayload::PartOf),
        EdgeType::ConflictsWith if no_options(requirement, threshold, polarity) => {
            Ok(EdgePayload::ConflictsWith)
        }
        EdgeType::SynergizesWith if no_options(requirement, threshold, polarity) => {
            Ok(EdgePayload::SynergizesWith)
        }
        _ => Err(invalid(
            "The supplied options do not apply to the selected edge kind.",
        )),
    }
}

fn no_options(
    requirement: Option<RequirementMode>,
    threshold: Option<f64>,
    polarity: Option<Polarity>,
) -> bool {
    requirement.is_none() && threshold.is_none() && polarity.is_none()
}

fn invalid(message: &'static str) -> human_errors::Error {
    human_errors::user(
        message,
        &[
            "Run `optimist edge create --help` and provide only fields belonging to the selected kind.",
        ],
    )
}

impl From<Polarity> for MeasurementPolarity {
    fn from(value: Polarity) -> Self {
        match value {
            Polarity::HigherIsBetter => Self::HigherIsBetter,
            Polarity::LowerIsBetter => Self::LowerIsBetter,
            Polarity::TargetRange => Self::TargetRange,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::EdgePayload;

    use super::{EdgeType, Polarity, RequirementMode, build};

    #[test]
    fn requires_explicit_kind_specific_fields() {
        assert!(build(EdgeType::Requires, None, None, None).is_err());
        assert!(matches!(
            build(
                EdgeType::Requires,
                Some(RequirementMode::Hard),
                Some(0.8),
                None
            ),
            Ok(EdgePayload::Requires(_))
        ));
        assert!(build(EdgeType::Measures, None, None, None).is_err());
        assert!(matches!(
            build(
                EdgeType::Measures,
                None,
                None,
                Some(Polarity::HigherIsBetter)
            ),
            Ok(EdgePayload::Measures(_))
        ));
    }
}
