//! Weighing a proposed change against the design it would replace.
//!
//! # What a comparison reports
//!
//! Two rankings of the same constraints, and the movement between them. Because
//! an intervention rebinds shared quantities without touching the structure of
//! the model, every difference is attributable to what it rebound.
//!
//! Movement is reported per constraint rather than as a single score. A change
//! almost never improves a design uniformly: adding a cache relieves the store
//! and loads the cache, sharding relieves each replica and multiplies the cost,
//! shedding protects the system and refuses customers. Collapsing that into one
//! number would hide the trade being made, which is the only part worth
//! discussing.
//!
//! # Reading the result
//!
//! The useful question is not whether utilisation fell but whether the
//! constraint that *bound* stopped binding, and what started binding instead.
//! Relieving the worst constraint usually promotes another, and a change that
//! merely moves the bottleneck somewhere less convenient is worth knowing about
//! before it is built.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    bottleneck::{Bottleneck, rank},
    evaluate::{EvaluationConfig, EvaluationError, evaluate_with_mutators},
    intervention::InterventionId,
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
};

/// How one constraint moved under a proposed change.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Movement {
    /// The component owning the constraint.
    pub component: ComponentId,
    /// The constraint's name within its component type.
    pub constraint: String,
    /// Mean utilisation before the change.
    pub before: f64,
    /// Mean utilisation after it.
    pub after: f64,
    /// Probability of binding before the change.
    pub bound_before: f64,
    /// Probability of binding after it.
    pub bound_after: f64,
}

impl Movement {
    /// Reports whether the change stopped this constraint from binding.
    pub fn relieved(&self) -> bool {
        self.bound_before > 0.0 && self.bound_after == 0.0
    }

    /// Reports whether the change caused this constraint to start binding.
    pub fn introduced(&self) -> bool {
        self.bound_before == 0.0 && self.bound_after > 0.0
    }

    /// Returns the change in mean utilisation, negative where load fell.
    pub fn shift(&self) -> f64 {
        self.after - self.before
    }
}

/// A design and a proposal, ranked and compared.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Comparison {
    /// Constraints of the unchanged design, worst first.
    pub baseline: Vec<Bottleneck>,
    /// Constraints once the change is applied, worst first.
    pub proposed: Vec<Bottleneck>,
    /// Per-constraint movement, largest improvement first.
    pub movements: Vec<Movement>,
}

impl Comparison {
    /// Returns the constraints the change stopped from binding.
    pub fn relieved(&self) -> Vec<&Movement> {
        self.movements
            .iter()
            .filter(|movement| movement.relieved())
            .collect()
    }

    /// Returns the constraints the change caused to start binding.
    ///
    /// Relieving the worst constraint usually promotes another, and a proposal
    /// that only moves the bottleneck is worth recognising before it is built.
    pub fn introduced(&self) -> Vec<&Movement> {
        self.movements
            .iter()
            .filter(|movement| movement.introduced())
            .collect()
    }
}

/// Solves a model with and without an intervention and reports the difference.
///
/// ```
/// use optimist::system::{EvaluationConfig, InterventionId, SystemModel, builtin_catalogue, compare};
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// scratchpad:
///   - name: peak_rate
///     expression: '900'
/// components:
///   - id: users
///     name: Users
///     type: client
///     properties:
///       request_rate: peak_rate
///   - id: api
///     name: API
///     type: compute
///     properties:
///       service_time: '0.02'
///       parallelism: '8'
/// relationships:
///   - from: users
///     to: api
/// interventions:
///   - id: quieter
///     name: Shift traffic away
///     overrides:
///       - name: peak_rate
///         expression: '200'
/// ")?;
///
/// let catalogue = builtin_catalogue()?;
/// let comparison = compare(
///     &model,
///     &catalogue,
///     &InterventionId::new("quieter"),
///     EvaluationConfig::default(),
/// )?;
///
/// // Eight slots at 20 ms sustain 400 per second: bound at 900, comfortable at 200.
/// assert!(comparison.baseline[0].binds());
/// assert!(!comparison.proposed[0].binds());
/// assert_eq!(comparison.relieved().len(), 1);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compare(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    intervention: &InterventionId,
    config: EvaluationConfig,
) -> Result<Comparison, EvaluationError> {
    compare_with_mutators(
        model,
        catalogue,
        &super::evaluate::builtin_mutators_or_empty(),
        intervention,
        config,
    )
}

/// Weighs an intervention against a caller's own set of behaviours.
///
/// The two runs must see identical behaviours as well as identical structure,
/// or the difference between them stops being attributable to the intervention.
pub fn compare_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, super::mutator::Mutator>,
    intervention: &InterventionId,
    config: EvaluationConfig,
) -> Result<Comparison, EvaluationError> {
    let overrides = model.intervention(intervention)?.bindings();
    let unchanged = BTreeMap::new();

    // The two scenarios share nothing but the model they read, so they are
    // solved side by side. Each is a long arithmetic run on one core, and a
    // comparison that took twice as long as the design it is about was spending
    // the second half of that wait on a machine that had cores to spare.
    let (baseline, proposed) = std::thread::scope(|scope| {
        let baseline = scope.spawn(|| ranked(model, catalogue, mutators, &unchanged, config));
        let proposed = scope.spawn(|| ranked(model, catalogue, mutators, &overrides, config));
        (
            baseline.join().expect("solving does not panic"),
            proposed.join().expect("solving does not panic"),
        )
    });
    let (baseline, proposed) = (baseline?, proposed?);

    Ok(Comparison {
        movements: movements(&baseline, &proposed),
        baseline,
        proposed,
    })
}

/// Weighs several proposals against one shared baseline.
///
/// Comparing proposals one at a time solves the unchanged design once for each
/// of them, which is the same answer every time: the baseline does not depend on
/// which proposal it is being weighed against. It is solved once here and shared,
/// so `n` proposals cost `n + 1` solves rather than `2n`, and the proposals are
/// solved alongside each other because none of them can observe another.
///
/// Every comparison therefore reads one baseline computed with one seed, which
/// is what makes two proposals as comparable with each other as each is with the
/// design they would replace.
///
/// ```
/// use optimist::system::{
///     EvaluationConfig, InterventionId, SystemModel, builtin_catalogue,
///     builtin_mutators, compare_many_with_mutators,
/// };
///
/// let model: SystemModel = serde_yaml_ng::from_str("
/// scratchpad:
///   - name: peak_rate
///     expression: '900'
/// components:
///   - id: users
///     name: Users
///     type: client
///     properties:
///       request_rate: peak_rate
///   - id: api
///     name: API
///     type: compute
///     properties:
///       service_time: '0.02'
///       parallelism: '8'
/// relationships:
///   - from: users
///     to: api
/// interventions:
///   - id: quieter
///     name: Shift traffic away
///     overrides:
///       - name: peak_rate
///         expression: '200'
///   - id: bigger
///     name: Add workers
///     overrides:
///       - name: peak_rate
///         expression: '400'
/// ")?;
///
/// let weighed = compare_many_with_mutators(
///     &model,
///     &builtin_catalogue()?,
///     &builtin_mutators()?,
///     &[InterventionId::new("quieter"), InterventionId::new("bigger")],
///     EvaluationConfig::default(),
/// )?;
///
/// assert_eq!(weighed.len(), 2);
/// // Both proposals were weighed against the very same ranking of the design.
/// assert_eq!(weighed[0].1.baseline[0].utilisation, weighed[1].1.baseline[0].utilisation);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compare_many_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, super::mutator::Mutator>,
    interventions: &[InterventionId],
    config: EvaluationConfig,
) -> Result<Vec<(InterventionId, Comparison)>, EvaluationError> {
    let unchanged = BTreeMap::new();
    let overrides = interventions
        .iter()
        .map(|intervention| Ok(model.intervention(intervention)?.bindings()))
        .collect::<Result<Vec<_>, EvaluationError>>()?;

    let (baseline, proposals) = std::thread::scope(|scope| {
        let baseline = scope.spawn(|| ranked(model, catalogue, mutators, &unchanged, config));
        let proposals = overrides
            .iter()
            .map(|overrides| scope.spawn(|| ranked(model, catalogue, mutators, overrides, config)))
            .collect::<Vec<_>>();
        (
            baseline.join().expect("solving does not panic"),
            proposals
                .into_iter()
                .map(|proposal| proposal.join().expect("solving does not panic"))
                .collect::<Vec<_>>(),
        )
    });
    let baseline = baseline?;

    interventions
        .iter()
        .zip(proposals)
        .map(|(intervention, proposed)| {
            let proposed = proposed?;
            Ok((
                intervention.clone(),
                Comparison {
                    movements: movements(&baseline, &proposed),
                    baseline: baseline.clone(),
                    proposed,
                },
            ))
        })
        .collect()
}

/// Solves one scenario and ranks what it is closest to exhausting.
fn ranked(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, super::mutator::Mutator>,
    overrides: &BTreeMap<String, String>,
    config: EvaluationConfig,
) -> Result<Vec<Bottleneck>, EvaluationError> {
    let settled = evaluate_with_mutators(model, catalogue, mutators, overrides, config)?;
    rank(
        model,
        catalogue,
        mutators,
        overrides,
        settled.settled(),
        config,
    )
}

fn movements(baseline: &[Bottleneck], proposed: &[Bottleneck]) -> Vec<Movement> {
    let key = |entry: &Bottleneck| (entry.component.clone(), entry.constraint.clone());
    let before = baseline
        .iter()
        .map(|entry| (key(entry), entry))
        .collect::<BTreeMap<_, _>>();
    let after = proposed
        .iter()
        .map(|entry| (key(entry), entry))
        .collect::<BTreeMap<_, _>>();
    let names = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut movements = names
        .into_iter()
        .filter_map(|name| {
            let (before, after) = (before.get(&name)?, after.get(&name)?);
            Some(Movement {
                component: name.0,
                constraint: name.1,
                before: before.utilisation,
                after: after.utilisation,
                bound_before: before.probability_of_binding,
                bound_after: after.probability_of_binding,
            })
        })
        .collect::<Vec<_>>();
    movements.sort_by(|left, right| {
        left.shift()
            .total_cmp(&right.shift())
            .then(left.component.as_str().cmp(right.component.as_str()))
            .then(left.constraint.cmp(&right.constraint))
    });
    movements
}
