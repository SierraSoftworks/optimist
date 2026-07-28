//! Finding and ranking the constraints a design is closest to exhausting.
//!
//! # What a bottleneck is
//!
//! Every constraint pairs a demand with the limit it consumes. Utilisation is
//! their ratio, and the constraint that binds is the one whose utilisation
//! reaches one first. Because both sides are carried as sample sets, the ratio
//! is taken per draw and the answer is a distribution rather than a single
//! figure.
//!
//! That distribution is the useful part. A constraint at 60% utilisation on
//! average may still exceed its limit in a fifth of draws, and a design whose
//! demand is uncertain enough will saturate long before its mean says it should.
//! The share of draws in which demand meets or exceeds the limit is reported
//! alongside the summary for exactly this reason:
//!
//! $$P(\text{bind}) = \frac{1}{n}\sum_{i=1}^{n} \mathbb{1}\{d_i \geq l_i\}$$
//!
//! # Ranking
//!
//! Constraints are ordered by how likely they are to bind, and by utilisation
//! where that likelihood ties. Ranking by probability rather than by mean
//! utilisation puts the constraint most exposed to a bad draw at the top, which
//! is the one worth spending on. Two constraints at the same average load are
//! not equally urgent if one of them is far more variable.
//!
//! The engine attaches no meaning to any constraint's name. A limit called
//! `iops` and one called `concurrency` are ranked by identical arithmetic, which
//! is what lets a new component type introduce a resource nobody anticipated and
//! still have it reported.

mod rank;

use std::collections::BTreeMap;

pub(super) use rank::rank;

use super::{
    evaluate::{EvaluationConfig, EvaluationError, Step, builtin_mutators_or_empty},
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
    mutator::Mutator,
};

/// How heavily one constraint is loaded in a solved model.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Bottleneck {
    /// The component owning the constraint.
    ///
    /// For a constraint belonging to a relationship this is the component the
    /// relationship leaves, and `link` below says which relationship.
    pub component: ComponentId,
    /// The relationship owning the constraint, where one does.
    ///
    /// A wire has limits of its own that belong to neither end: how fast it
    /// carries bytes is a fact about the link, and attributing it to either
    /// component would send somebody to resize the wrong thing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// The constraint's name within its component type.
    pub constraint: String,
    /// What saturating this constraint does to the system.
    pub summary: String,
    /// Replicas of the owning component across every enclosing scale unit.
    ///
    /// The figures below describe one replica, so a constraint that binds does
    /// so in each of these copies.
    pub replicas: f64,
    /// Mean of demand over limit.
    pub utilisation: f64,
    /// Utilisation at the ninetieth percentile of draws.
    pub utilisation_p90: f64,
    /// Share of draws in which demand meets or exceeds the limit.
    pub probability_of_binding: f64,
    /// Mean limit less mean demand, in the constraint's own units.
    pub headroom: f64,
}

impl Bottleneck {
    /// Reports whether this constraint binds in any draw.
    pub fn binds(&self) -> bool {
        self.probability_of_binding > 0.0
    }
}

/// Evaluates every constraint in a solved model, worst first.
///
/// ```
/// use optimist::system::{
///     Component, ComponentId, EvaluationConfig, Relationship, SystemModel,
///     builtin_catalogue, bottlenecks, evaluate,
/// };
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// components:
///   - id: users
///     name: Users
///     type: client
///     properties:
///       request_rate: '900'
///   - id: api
///     name: API
///     type: compute
///     properties:
///       service_time: '0.02'
///       parallelism: '8'
/// relationships:
///   - from: users
///     to: api
/// ")?;
/// let catalogue = builtin_catalogue()?;
/// let config = EvaluationConfig::default();
/// let evaluation = evaluate(&model, &catalogue, config)?;
/// let ranked = bottlenecks(&model, &catalogue, evaluation.settled(), config)?;
///
/// // Eight slots at 20 ms sustain 400 requests per second against 900 offered.
/// assert_eq!(ranked[0].constraint, "capacity");
/// assert!(ranked[0].binds());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn bottlenecks(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    step: &Step,
    config: EvaluationConfig,
) -> Result<Vec<Bottleneck>, EvaluationError> {
    bottlenecks_with_mutators(model, catalogue, &builtin_mutators_or_empty(), step, config)
}

/// Ranks constraints against a caller's own set of behaviours.
///
/// A design may define behaviours the shipped catalogue never anticipated, and
/// a constraint's demand can be read from a flow one of them rewrote. Ranking
/// against the shipped set alone would silently drop those rewrites and report
/// a load the design does not actually place.
pub fn bottlenecks_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    step: &Step,
    config: EvaluationConfig,
) -> Result<Vec<Bottleneck>, EvaluationError> {
    rank(model, catalogue, mutators, &BTreeMap::new(), step, config)
}
