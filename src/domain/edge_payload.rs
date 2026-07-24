use serde::{Deserialize, Deserializer, Serialize, de};

use super::{
    Duration, EdgeKind, EffectTransience, Estimate, MeasurementCalibration,
    MeasurementCalibrationError, QuantityValue, SignedInfluence, Unit,
};

/// Unit-aware counterfactual anchor pair for one local linear response.
///
/// If the source moves by `source_change`, the destination is expected to move by
/// the uncertain `destination_change`. Simulation samples the local coefficient
/// $\beta=\Delta y/\Delta x$ and applies it to source movement from baseline.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LinearResponse {
    /// Finite nonzero source movement expressed in [`Self::source_unit`].
    pub source_change: f64,
    /// Canonical unit of the source movement.
    pub source_unit: Unit,
    /// Uncertain destination movement; deltas may be negative regardless of level support.
    pub destination_change: Estimate<QuantityValue>,
    /// Canonical unit of the destination movement.
    pub destination_unit: Unit,
}

/// Uncertain local causal effect embedded in a `contributes` or `changes` edge.
///
/// The response describes how strongly the destination moves. The profile
/// describes for how long, so a time-boxed intervention can be modelled without
/// a placeholder node standing in for its own expiry.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CausalEffect {
    /// Counterfactual anchor pair defining the uncertain local slope.
    pub response: LinearResponse,
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
    response: LinearResponse,
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
        Self::linear(value.response, value.lag, value.mechanism, value.evidence)
            .map(|effect| effect.with_transience(value.transience.map(|value| *value)))
            .map_err(de::Error::custom)
    }
}

impl CausalEffect {
    /// Creates a unit-aware local linear response after validating its anchor.
    ///
    /// The effect is permanent until [`Self::with_transience`] shapes it.
    pub fn linear(
        response: LinearResponse,
        lag: Option<Estimate<Duration>>,
        mechanism: String,
        evidence: Vec<String>,
    ) -> Result<Self, CausalResponseError> {
        if !response.source_change.is_finite() || response.source_change == 0.0 {
            return Err(CausalResponseError::InvalidSourceChange);
        }
        Ok(Self {
            response,
            transience: None,
            lag,
            mechanism,
            evidence,
        })
    }

    /// Applies transient behaviour, or restores a permanent effect with `None`.
    #[must_use]
    pub fn with_transience(mut self, transience: Option<EffectTransience>) -> Self {
        self.transience = transience.map(Box::new);
        self
    }
}

/// Invalid counterfactual response anchors.
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum CausalResponseError {
    /// A local slope cannot be derived from a zero or non-finite source movement.
    #[error("a linear response requires a finite nonzero source change")]
    InvalidSourceChange,
}

#[cfg(test)]
mod causal_response_tests {
    use super::*;
    use crate::domain::{Distribution, EstimateId};

    #[test]
    fn linear_response_round_trips_and_rejects_zero_source_change() {
        let response = LinearResponse {
            source_change: 2.0,
            source_unit: Unit::base("day").unwrap(),
            destination_change: Estimate::<QuantityValue>::new(
                EstimateId::new(0),
                Distribution::point(-1.0).unwrap(),
            )
            .unwrap(),
            destination_unit: Unit::base("incident").unwrap(),
        };
        let value = CausalEffect::linear(response, None, String::new(), vec![]).unwrap();
        let mut json = serde_json::to_value(&value).unwrap();
        assert_eq!(
            serde_json::from_value::<CausalEffect>(json.clone()).unwrap(),
            value
        );
        json["response"]["source_change"] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<CausalEffect>(json).is_err());
    }

    #[test]
    fn rejects_normalized_effect_storage() {
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
