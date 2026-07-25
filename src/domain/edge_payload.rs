use serde::{Deserialize, Deserializer, Serialize};

use super::{
    Duration, EdgeKind, EffectTransience, Elasticity, Estimate, MeasurementCalibration,
    MeasurementCalibrationError, SignedInfluence,
};

/// Uncertain proportional causal effect embedded in a `contributes` or `changes` edge.
///
/// The response says how strongly the destination moves, the profile says for how
/// long, and the mechanism says why. Strength is a dimensionless ratio rather than
/// a movement in the destination's unit, so one relationship can connect
/// quantities measured in different things without the author converting between
/// them, and re-baselining either endpoint leaves the claim intact.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CausalEffect {
    /// Dimensionless proportional response.
    ///
    /// On a `contributes` edge this is an elasticity: multiplying the source by
    /// `r` multiplies the destination by `r^response`. On a `changes` edge the
    /// source has no level to take a ratio of, so it is instead the multiplier
    /// applied while the intervention is fully active.
    pub response: Estimate<Elasticity>,
    /// Temporal shape and rebound; absent leaves the effect permanent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transience: Option<Box<EffectTransience>>,
    /// Optional non-negative delay before the effect reaches its destination.
    pub lag: Option<Estimate<Duration>>,
    /// Markdown explanation of the causal mechanism, boundaries, and assumptions.
    pub mechanism: String,
    /// Aggregate-local evidence references supporting this relationship.
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CausalEffectWire {
    response: Estimate<Elasticity>,
    #[serde(default)]
    transience: Option<Box<EffectTransience>>,
    lag: Option<Estimate<Duration>>,
    mechanism: String,
    #[serde(default)]
    evidence: Vec<String>,
}

impl<'de> Deserialize<'de> for CausalEffect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = CausalEffectWire::deserialize(deserializer)?;
        Ok(
            Self::proportional(value.response, value.lag, value.mechanism, value.evidence)
                .with_transience(value.transience.map(|value| *value)),
        )
    }
}

impl CausalEffect {
    /// Creates a proportional causal effect.
    ///
    /// The effect is permanent until [`Self::with_transience`] shapes it.
    pub fn proportional(
        response: Estimate<Elasticity>,
        lag: Option<Estimate<Duration>>,
        mechanism: String,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            response,
            transience: None,
            lag,
            mechanism,
            evidence,
        }
    }

    /// Applies transient behaviour, or restores a permanent effect with `None`.
    #[must_use]
    pub fn with_transience(mut self, transience: Option<EffectTransience>) -> Self {
        self.transience = transience.map(Box::new);
        self
    }
}

#[cfg(test)]
mod causal_response_tests {
    use super::*;
    use crate::domain::{Distribution, EstimateId};

    #[test]
    fn proportional_response_round_trips() {
        let value = CausalEffect::proportional(
            Estimate::<Elasticity>::new(EstimateId::new(0), Distribution::point(-0.8).unwrap())
                .unwrap(),
            None,
            String::new(),
            vec![],
        );
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(serde_json::from_value::<CausalEffect>(json).unwrap(), value);
    }

    #[test]
    fn rejects_unit_bearing_and_normalized_response_storage() {
        // The pre-ratio anchor pair carried units on both sides.
        assert!(
            serde_json::from_value::<CausalEffect>(serde_json::json!({
                "response": {
                    "source_change": 1.0,
                    "source_unit": {},
                    "destination_change": { "id": "A", "revision": 0, "source": {
                        "type": "squiggle",
                        "definition": { "source": "pointMass(1)", "seed": 42, "sample_count": 256, "target_unit": {} }
                    } },
                    "destination_unit": {}
                },
                "lag": null,
                "mechanism": "",
                "evidence": []
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CausalEffect>(serde_json::json!({
                "effect": {
                    "id": "A",
                    "revision": 0,
                    "distribution": { "type": "point", "value": 0.5 }
                },
                "lag": null,
                "mechanism": "",
                "evidence": []
            }))
            .is_err()
        );
    }
}

/// One immutable quantitative reading embedded in a [`Measurement`] edge payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Observation {
    /// Measurement-edge-local identifier used for correction references.
    pub id: u64,
    /// Optimistic-concurrency revision of this reading.
    pub revision: u64,
    /// Observed numeric value expressed in [`Observation::unit`].
    pub value: f64,
    /// Unit recorded at collection time.
    pub unit: String,
    /// RFC 3339 timestamp or project-approved temporal representation.
    pub observed_at: String,
    /// Person, system, query, or citation which produced the reading.
    pub source: String,
    /// Known measurement-error standard deviation; `None` means unknown.
    pub measurement_standard_deviation: Option<f64>,
    /// Earlier observation replaced by this correction, if any.
    pub supersedes: Option<u64>,
}

/// Maps metric movement to desirability for its measured subject.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementPolarity {
    /// Larger readings indicate a better subject state.
    HigherIsBetter,
    /// Smaller readings indicate a better subject state.
    LowerIsBetter,
    /// Readings are best inside a separately configured interval.
    TargetRange,
}

/// Metric-to-subject measurement data owned by a `measures` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Measurement {
    /// Interpretation of movement when mapping readings to subject state.
    pub polarity: MeasurementPolarity,
    /// Optional anchors translating metric readings into normalized subject state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<MeasurementCalibration>,
    /// Append-only readings for this exact metric/subject pair.
    #[serde(default)]
    pub observations: Vec<Observation>,
}

impl Measurement {
    /// Validates and replaces the reading-to-state calibration for this relationship.
    pub fn set_calibration(
        &mut self,
        calibration: Option<MeasurementCalibration>,
    ) -> Result<(), MeasurementCalibrationError> {
        let calibration = calibration
            .map(MeasurementCalibration::validated)
            .transpose()?;
        let compatible = match (&self.polarity, &calibration) {
            (_, None) => true,
            (
                MeasurementPolarity::HigherIsBetter,
                Some(MeasurementCalibration::Linear {
                    state_zero,
                    state_one,
                }),
            ) => state_zero < state_one,
            (
                MeasurementPolarity::LowerIsBetter,
                Some(MeasurementCalibration::Linear {
                    state_zero,
                    state_one,
                }),
            ) => state_zero > state_one,
            (
                MeasurementPolarity::TargetRange,
                Some(MeasurementCalibration::TargetRange { .. }),
            ) => true,
            _ => false,
        };
        if !compatible {
            return Err(MeasurementCalibrationError::PolarityMismatch(self.polarity));
        }
        self.calibration = calibration;
        Ok(())
    }
}

/// Prerequisite semantics embedded in a `requires` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Requirement {
    /// Whether an unsatisfied prerequisite makes its source infeasible.
    pub hard: bool,
    /// Optional normalized state at which the prerequisite is considered satisfied.
    pub satisfaction_threshold: Option<f64>,
}

/// Uncertain blocking strength embedded in a `blocks` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct BlockingEffect {
    /// Signed normalized effect; negative values normally represent inhibition.
    pub degree: Estimate<SignedInfluence>,
}

/// Type-safe payload defining an edge's semantics and embedded owned values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "properties", rename_all = "snake_case")]
pub enum EdgePayload {
    /// Signed causal influence between system conditions or results.
    Contributes(CausalEffect),
    /// Measurement definition and readings for a metric/subject pair.
    Measures(Measurement),
    /// Expected intervention effect on a factor.
    Changes(CausalEffect),
    /// Hard or soft prerequisite semantics.
    Requires(Requirement),
    /// Non-causal factor hierarchy with no additional fields.
    PartOf,
    /// Blocking effect exerted by the source factor.
    Blocks(BlockingEffect),
    /// Symmetric intervention incompatibility.
    ConflictsWith,
    /// Symmetric intervention synergy.
    SynergizesWith,
}

impl EdgePayload {
    /// Returns the structural relationship kind implied by this payload variant.
    pub const fn kind(&self) -> EdgeKind {
        match self {
            Self::Contributes(_) => EdgeKind::Contributes,
            Self::Measures(_) => EdgeKind::Measures,
            Self::Changes(_) => EdgeKind::Changes,
            Self::Requires(_) => EdgeKind::Requires,
            Self::PartOf => EdgeKind::PartOf,
            Self::Blocks(_) => EdgeKind::Blocks,
            Self::ConflictsWith => EdgeKind::ConflictsWith,
            Self::SynergizesWith => EdgeKind::SynergizesWith,
        }
    }
}
