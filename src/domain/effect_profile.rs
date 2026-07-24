use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{Duration, Estimate, QuantityValue};

/// How a transient effect returns toward zero once its hold window ends.
///
/// Release begins in the period immediately after the final held period, so
/// [`Self::Immediate`] reproduces a rectangular pulse of exactly the hold width.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectRelease {
    /// The effect vanishes entirely in the first period after its hold window.
    #[default]
    Immediate,
    /// The effect declines linearly and reaches exactly zero after `over` periods.
    Linear {
        /// Non-negative planning periods spanned by the decline.
        over: Estimate<Duration>,
    },
    /// The effect decays geometrically and approaches, but never reaches, zero.
    Exponential {
        /// Planning periods over which the remaining effect halves.
        half_life: Estimate<Duration>,
    },
}

impl EffectRelease {
    /// Reports whether the effect stops without an intermediate decline.
    pub fn is_immediate(&self) -> bool {
        matches!(self, Self::Immediate)
    }
}

/// A transient rebound triggered when a primary effect starts releasing.
///
/// Ending an intervention is itself an event: work suppressed during a change
/// freeze returns as a backlog, and reverting a mitigation can briefly overshoot
/// the original baseline. The rebound carries its own magnitude estimate so it is
/// never forced to be a fixed multiple of the effect it follows.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectAftereffect {
    /// Periods held at full rebound strength; `None` makes the rebound permanent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Estimate<Duration>>,
    /// How the rebound itself subsides once its hold window ends.
    pub release: EffectRelease,
}

/// Temporal shape of an intervention effect across the planning horizon.
///
/// The default profile is persistent: the effect ramps instantly, holds forever,
/// and never rebounds. That reproduces a monotone step function, so omitting a
/// profile leaves projections unchanged. A bounded `hold` turns the effect into a
/// pulse, which is how time-boxed interventions such as change freezes, embargoes,
/// or temporary staffing are modelled without inventing placeholder nodes.
///
/// Durations are uncertain and sampled per Monte Carlo draw, then rounded up to
/// whole planning periods; a profile therefore describes a distribution over
/// shapes rather than one fixed schedule.
///
/// ```
/// use optimist::domain::{
///     Duration, EffectAftereffect, EffectProfile, EffectRelease, Estimate, EstimateId,
///     SquiggleEstimateDefinition, Unit,
/// };
///
/// fn periods(source: &str) -> Result<Estimate<Duration>, Box<dyn std::error::Error>> {
///     let unit = Unit::base("duration")?;
///     Ok(Estimate::<Duration>::from_squiggle(
///         EstimateId::new(0),
///         SquiggleEstimateDefinition {
///             source: source.to_owned(),
///             seed: 42,
///             sample_count: 256,
///             target_unit: unit.clone(),
///         },
///         &unit,
///     )?)
/// }
///
/// // A change freeze held for two periods that ends abruptly and leaves one
/// // period of backlog rebound behind it.
/// let profile = EffectProfile::new(
///     None,
///     Some(periods("pointMass(2)")?),
///     EffectRelease::Immediate,
///     Some(EffectAftereffect {
///         hold: Some(periods("pointMass(1)")?),
///         release: EffectRelease::Immediate,
///     }),
/// )?;
/// assert!(!profile.is_persistent());
/// assert!(EffectProfile::default().is_persistent());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct EffectProfile {
    /// Periods spent rising to full strength; `None` reaches full strength at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ramp: Option<Estimate<Duration>>,
    /// Periods held at full strength; `None` makes the effect permanent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Estimate<Duration>>,
    /// How the effect subsides once its hold window ends.
    #[serde(default, skip_serializing_if = "EffectRelease::is_immediate")]
    pub release: EffectRelease,
    /// Optional transient rebound triggered when the release begins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aftereffect: Option<EffectAftereffect>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectProfileWire {
    #[serde(default)]
    ramp: Option<Estimate<Duration>>,
    #[serde(default)]
    hold: Option<Estimate<Duration>>,
    #[serde(default)]
    release: EffectRelease,
    #[serde(default)]
    aftereffect: Option<EffectAftereffect>,
}

impl<'de> Deserialize<'de> for EffectProfile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = EffectProfileWire::deserialize(deserializer)?;
        Self::new(value.ramp, value.hold, value.release, value.aftereffect)
            .map_err(de::Error::custom)
    }
}

impl EffectProfile {
    /// Creates a temporal profile after rejecting unreachable release phases.
    pub fn new(
        ramp: Option<Estimate<Duration>>,
        hold: Option<Estimate<Duration>>,
        release: EffectRelease,
        aftereffect: Option<EffectAftereffect>,
    ) -> Result<Self, EffectProfileError> {
        if hold.is_none() && (!release.is_immediate() || aftereffect.is_some()) {
            return Err(EffectProfileError::PermanentEffectCannotRelease);
        }
        Ok(Self {
            ramp,
            hold,
            release,
            aftereffect,
        })
    }

    /// Reports whether this profile is the persistent step applied by default.
    ///
    /// Persistent profiles consume no randomness during projection, which keeps
    /// sampling streams identical to projects that declare no profile at all.
    pub fn is_persistent(&self) -> bool {
        self.ramp.is_none()
            && self.hold.is_none()
            && self.release.is_immediate()
            && self.aftereffect.is_none()
    }
}

/// Temporal profiles that describe a phase which can never be reached.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EffectProfileError {
    /// A profile without a hold window never releases, so it cannot decline or rebound.
    #[error("a permanent effect cannot declare a release form or an aftereffect")]
    PermanentEffectCannotRelease,
    /// A rebound magnitude and an aftereffect were not declared together.
    #[error("a rebound magnitude requires an aftereffect, and an aftereffect requires a magnitude")]
    MismatchedAftereffect,
    /// Transient behaviour was declared without any departure from a permanent step.
    #[error("transient behaviour requires a ramp, a hold window, or an aftereffect")]
    PermanentTransience,
}

/// Complete transient behaviour applied to one intervention effect.
///
/// Shape and rebound magnitude travel together because neither is meaningful
/// alone, and both are absent for the permanent effects that need no schedule.
/// Effects carry this as one optional value so a permanent relationship costs
/// nothing to represent.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EffectTransience {
    /// Temporal shape applied to the effect across the planning horizon.
    pub profile: EffectProfile,
    /// Destination movement contributed by the rebound once the effect releases.
    ///
    /// Held separately from the response because ending an intervention is its
    /// own event: a drained backlog rarely returns exactly what was withheld.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebound: Option<Estimate<QuantityValue>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectTransienceWire {
    profile: EffectProfile,
    #[serde(default)]
    rebound: Option<Estimate<QuantityValue>>,
}

impl<'de> Deserialize<'de> for EffectTransience {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = EffectTransienceWire::deserialize(deserializer)?;
        Self::new(value.profile, value.rebound).map_err(de::Error::custom)
    }
}

impl EffectTransience {
    /// Pairs a temporal shape with the rebound magnitude it triggers.
    ///
    /// A rebound magnitude without an aftereffect never fires, an aftereffect
    /// without a magnitude moves nothing, and a wholly persistent profile is
    /// simply a permanent effect. Rejecting all three surfaces a half-finished
    /// model instead of silently projecting a shape nobody authored.
    pub fn new(
        profile: EffectProfile,
        rebound: Option<Estimate<QuantityValue>>,
    ) -> Result<Self, EffectProfileError> {
        if rebound.is_some() != profile.aftereffect.is_some() {
            return Err(EffectProfileError::MismatchedAftereffect);
        }
        if profile.is_persistent() {
            return Err(EffectProfileError::PermanentTransience);
        }
        Ok(Self { profile, rebound })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EstimateId, SquiggleEstimateDefinition, Unit};

    fn periods(source: &str) -> Estimate<Duration> {
        let unit = Unit::base("duration").unwrap();
        Estimate::<Duration>::from_squiggle(
            EstimateId::new(0),
            SquiggleEstimateDefinition {
                source: source.to_owned(),
                seed: 42,
                sample_count: 256,
                target_unit: unit.clone(),
            },
            &unit,
        )
        .unwrap()
    }

    #[test]
    fn default_profile_is_persistent_and_serializes_to_an_empty_document() {
        let profile = EffectProfile::default();
        assert!(profile.is_persistent());
        assert_eq!(
            serde_json::to_value(&profile).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(
            serde_json::from_value::<EffectProfile>(serde_json::json!({})).unwrap(),
            profile
        );
    }

    #[test]
    fn pulse_profile_round_trips() {
        let profile = EffectProfile::new(
            None,
            Some(periods("pointMass(2)")),
            EffectRelease::Linear {
                over: periods("pointMass(3)"),
            },
            Some(EffectAftereffect {
                hold: Some(periods("pointMass(1)")),
                release: EffectRelease::Immediate,
            }),
        )
        .unwrap();
        let json = serde_json::to_value(&profile).unwrap();
        assert_eq!(
            serde_json::from_value::<EffectProfile>(json).unwrap(),
            profile
        );
        assert!(!profile.is_persistent());
    }

    #[test]
    fn rejects_release_and_aftereffect_without_a_hold_window() {
        assert_eq!(
            EffectProfile::new(
                None,
                None,
                EffectRelease::Linear {
                    over: periods("pointMass(1)"),
                },
                None,
            ),
            Err(EffectProfileError::PermanentEffectCannotRelease)
        );
        assert_eq!(
            EffectProfile::new(
                None,
                None,
                EffectRelease::Immediate,
                Some(EffectAftereffect {
                    hold: None,
                    release: EffectRelease::Immediate,
                }),
            ),
            Err(EffectProfileError::PermanentEffectCannotRelease)
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(
            serde_json::from_value::<EffectProfile>(serde_json::json!({ "decay": 1 })).is_err()
        );
    }
}
