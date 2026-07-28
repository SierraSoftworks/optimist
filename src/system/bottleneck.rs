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

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::squiggle::Value;

use super::{
    compile::{Timing, prepare, runtime},
    evaluate::{EvaluationConfig, EvaluationError, Step},
    expression::{INBOUND, OUTBOUND, PREVIOUS, STEP, TIME},
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
    values::draws,
};

/// How heavily one constraint is loaded in a solved model.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Bottleneck {
    /// The component owning the constraint.
    pub component: ComponentId,
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
    rank(model, catalogue, &BTreeMap::new(), step, config)
}

pub(super) fn rank(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    overrides: &BTreeMap<String, String>,
    step: &Step,
    config: EvaluationConfig,
) -> Result<Vec<Bottleneck>, EvaluationError> {
    let plan = prepare(
        model,
        catalogue,
        &super::evaluate::builtin_mutators_or_empty(),
        overrides,
        Timing {
            seed: config.seed,
            sample_count: config.sample_count,
            time: step.time,
            step: config.step,
        },
    )?;
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let mut ranked = Vec::new();
    for component in &plan.components {
        let Some(state) = step.components.get(&component.id) else {
            continue;
        };
        let mut scope = plan.globals.clone();
        scope.extend(component.properties.clone());
        scope.extend(state.channels.clone());
        scope.insert(TIME.to_owned(), Value::Number(step.time));
        scope.insert(STEP.to_owned(), Value::Number(config.step));
        scope.insert(INBOUND.to_owned(), Value::Dictionary(BTreeMap::new()));
        scope.insert(OUTBOUND.to_owned(), Value::Dictionary(BTreeMap::new()));
        scope.insert(PREVIOUS.to_owned(), Value::Dictionary(BTreeMap::new()));

        let mut runtime = runtime(config.seed, config.sample_count)?;
        for (name, (demand, limit)) in &component.constraints {
            let location = || format!("constraint '{name}' of component '{}'", component.id);
            let bindings = || {
                scope
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.clone()))
            };
            let demand = runtime
                .evaluate_values(demand, bindings())
                .map_err(|error| EvaluationError::Evaluation {
                    location: location(),
                    message: error.message,
                })?;
            let limit = runtime
                .evaluate_values(limit, bindings())
                .map_err(|error| EvaluationError::Evaluation {
                    location: location(),
                    message: error.message,
                })?;
            let (Some(demand), Some(limit)) = (
                draws(&demand, config.sample_count, &mut rng),
                draws(&limit, config.sample_count, &mut rng),
            ) else {
                continue;
            };
            ranked.push(measure(
                component.id.clone(),
                name.clone(),
                component.component_type.constraints[name].summary.clone(),
                component.replicas,
                &demand,
                &limit,
            ));
        }
    }
    ranked.sort_by(|left, right| {
        right
            .probability_of_binding
            .total_cmp(&left.probability_of_binding)
            .then(right.utilisation.total_cmp(&left.utilisation))
            .then(left.component.as_str().cmp(right.component.as_str()))
            .then(left.constraint.cmp(&right.constraint))
    });
    Ok(ranked)
}

fn measure(
    component: ComponentId,
    constraint: String,
    summary: String,
    replicas: f64,
    demand: &[f64],
    limit: &[f64],
) -> Bottleneck {
    let count = demand.len().min(limit.len()).max(1);
    let mut ratios = Vec::with_capacity(count);
    let mut binding = 0_usize;
    let mut demand_total = 0.0;
    let mut limit_total = 0.0;
    for (demand, limit) in demand.iter().zip(limit).take(count) {
        demand_total += demand;
        limit_total += limit;
        if demand >= limit {
            binding += 1;
        }
        // A limit of zero admits no demand at all, so any demand against it is
        // fully saturating rather than undefined.
        ratios.push(if *limit == 0.0 {
            f64::from(u8::from(*demand > 0.0))
        } else {
            demand / limit
        });
    }
    ratios.sort_by(f64::total_cmp);
    let utilisation = ratios.iter().sum::<f64>() / count as f64;
    let index = ((count as f64 * 0.9).ceil() as usize).clamp(1, count) - 1;
    Bottleneck {
        component,
        constraint,
        summary,
        replicas,
        utilisation,
        utilisation_p90: ratios[index],
        probability_of_binding: binding as f64 / count as f64,
        headroom: (limit_total - demand_total) / count as f64,
    }
}
