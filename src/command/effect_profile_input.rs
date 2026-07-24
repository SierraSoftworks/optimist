use serde::{Deserialize, Serialize};

use crate::domain::SquiggleEstimateDefinition;

/// Authored form describing how a transient effect subsides.
///
/// Every duration is a Squiggle program in the synthetic `duration` unit, so a
/// release span is as uncertain as any other estimate in the project.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectReleaseInput {
    /// The effect stops entirely in the first period after its hold window.
    #[default]
    Immediate,
    /// The effect declines linearly and reaches zero after `over` periods.
    Linear {
        /// Non-negative planning periods spanned by the decline.
        over: SquiggleEstimateDefinition,
    },
    /// The effect decays geometrically and approaches, but never reaches, zero.
    Exponential {
        /// Planning periods over which the remaining effect halves.
        half_life: SquiggleEstimateDefinition,
    },
}

/// Authored rebound applied when a transient effect starts releasing.
///
/// The magnitude and the timing are declared together because a rebound with no
/// magnitude moves nothing and a magnitude with no rebound never fires.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectAftereffectInput {
    /// Destination movement contributed by the rebound in the destination's native unit.
    pub magnitude: SquiggleEstimateDefinition,
    /// Periods held at full rebound strength; omitted makes the rebound permanent.
    #[serde(default)]
    pub hold: Option<SquiggleEstimateDefinition>,
    /// How the rebound itself subsides once its hold window ends.
    #[serde(default)]
    pub release: EffectReleaseInput,
}

/// Authored temporal shape for one intervention effect.
///
/// Omitting `hold` leaves the effect permanent, which is the shape applied when no
/// profile is configured at all. A bounded `hold` turns the effect into a pulse,
/// which is how time-boxed interventions are expressed without placeholder nodes.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectProfileInput {
    /// Periods spent rising to full strength; omitted reaches full strength at once.
    #[serde(default)]
    pub ramp: Option<SquiggleEstimateDefinition>,
    /// Periods held at full strength; omitted makes the effect permanent.
    #[serde(default)]
    pub hold: Option<SquiggleEstimateDefinition>,
    /// How the effect subsides once its hold window ends.
    #[serde(default)]
    pub release: EffectReleaseInput,
    /// Optional transient rebound triggered when the release begins.
    #[serde(default)]
    pub aftereffect: Option<EffectAftereffectInput>,
}
