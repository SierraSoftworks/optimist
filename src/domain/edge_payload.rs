use serde::{Deserialize, Serialize};

use super::{Duration, EdgeKind, Estimate, SignedInfluence};

/// Uncertain local causal effect embedded in a `contributes` or `changes` edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CausalEffect {
    /// Signed probability distribution for direction and normalized local strength.
    pub effect: Estimate<SignedInfluence>,
    /// Optional non-negative delay before the effect reaches its destination.
    pub lag: Option<Estimate<Duration>>,
    /// Markdown explanation of the causal mechanism, boundaries, and assumptions.
    pub mechanism: String,
    /// Aggregate-local evidence references supporting this relationship.
    #[serde(default)]
    pub evidence: Vec<String>,
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
    /// Append-only readings for this exact metric/subject pair.
    #[serde(default)]
    pub observations: Vec<Observation>,
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
