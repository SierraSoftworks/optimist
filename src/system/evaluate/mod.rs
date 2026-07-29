//! Solving a system model for the quantities flowing through it.
//!
//! # Why iteration rather than ordering
//!
//! Component channels can be arranged in dependency order only when the model is
//! acyclic, and the models worth building rarely are. Utilisation sets queueing
//! delay, delay sets how long each request occupies a worker, occupancy sets
//! utilisation again. That loop has no first term to evaluate.
//!
//! The solver therefore relaxes toward a fixed point: it evaluates every
//! component against the current estimate of its inputs, blends the result part
//! of the way toward what it computed, and repeats until nothing moves. Where
//! the model happens to be acyclic this converges immediately, so ordering is
//! never needed as a special case.
//!
//! # Relaxation over sample sets
//!
//! Evaluation is elementwise across aligned draws, so each draw index carries a
//! deterministic system and settles on its own fixed point independently of the
//! others. Uncertainty is therefore not smeared through the loop: where demand
//! is uncertain enough that some draws saturate and others do not, the converged
//! result is a genuine mixture and its spread reports exactly how much of the
//! distribution has crossed into congestion.
//!
//! A single set of starting values is used, so where a loop admits more than one
//! fixed point the solver reports the one reachable from rest. That is the lower,
//! uncongested branch of a bistable system. The congested branch exists and is
//! not searched for; a wide converged distribution is the signal that the system
//! is operating near the fold between them.
//!
//! # Settling on several states is also a result
//!
//! Past a fold the draws divide between branches, and a draw on a branch too
//! steep for the damped step to follow swaps between values indefinitely. The
//! per-draw test then never passes although the ensemble has been still for
//! hundreds of passes. The solver therefore also asks whether the *distribution*
//! has stopped moving, and where it has, reports the step as settled on a
//! mixture and says how many states its draws divided between.
//!
//! # Not converging is a result
//!
//! An iteration that never settles — in draws or in distribution — is reported
//! rather than hidden behind a last iterate. A loop whose gain exceeds one has
//! no steady state to find, and saying so is more useful than returning
//! whichever values the cap happened to stop at. The quantity still moving is
//! named alongside the values so a caller can tell a wholly unstable system from
//! one unstable in its tail.

mod aggregate;
mod arrivals;
mod blend;
mod component;
mod config;
mod error;
mod flow;
mod merge;
mod modes;
mod mutate;
pub mod progress;
mod queue;
mod relax;
mod state;
mod stationary;
mod transfer;

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rayon::prelude::*;

use crate::profile::time;

pub use config::{EvaluationConfig, SolveMode};
pub use error::EvaluationError;
pub use state::{ComponentState, Evaluation, LinkId, LinkState, Mixture, Step, Unsettled};

use crate::system::{
    compile::{Timing, prepare},
    intervention::InterventionId,
    manifest::ComponentType,
    model::{ComponentId, SystemModel},
    mutator::Mutator,
};

use progress::Reporting;
use relax::relax;

/// Solves a model against a catalogue of component types.
pub fn evaluate(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    solved(
        model,
        catalogue,
        &builtin_mutators_or_empty(),
        &BTreeMap::new(),
        config,
        Reporting::to(None),
    )
}

/// Solves a model with one of its interventions applied.
///
/// The model is otherwise untouched, so any difference from the baseline is
/// attributable to the quantities the intervention rebinds.
pub fn evaluate_intervention(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    intervention: &InterventionId,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    let overrides = model.intervention(intervention)?.bindings();
    solved(
        model,
        catalogue,
        &builtin_mutators_or_empty(),
        &overrides,
        config,
        Reporting::to(None),
    )
}

/// Solves a model with one of its interventions applied, against explicit
/// behaviours.
///
/// A design may define behaviours the shipped catalogue never anticipated, and
/// solving without them would quietly drop the rewrites they apply to the flows
/// travelling along a relationship.
#[deprecated(since = "0.1.0", note = "use `Solve::intervention` and `Solve::evaluate`")]
pub fn evaluate_intervention_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    intervention: &InterventionId,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    let overrides = model.intervention(intervention)?.bindings();
    solved(
        model,
        catalogue,
        mutators,
        &overrides,
        config,
        Reporting::to(None),
    )
}

/// Solves a model against explicit catalogues and scratchpad replacements.
#[deprecated(since = "0.1.0", note = "use `Solve::overrides` and `Solve::evaluate`")]
pub fn evaluate_with_mutators(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: EvaluationConfig,
) -> Result<Evaluation, EvaluationError> {
    solved(
        model,
        catalogue,
        mutators,
        overrides,
        config,
        Reporting::to(None),
    )
}

/// Solves a model, telling a reporter how it is getting on.
pub(super) fn solved(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: EvaluationConfig,
    reporting: Reporting<'_>,
) -> Result<Evaluation, EvaluationError> {
    let shares: Vec<EvaluationConfig> = config.divided().collect();
    if let [whole] = shares.as_slice() {
        return horizon(model, catalogue, mutators, overrides, *whole, reporting);
    }
    // Every draw index carries an independent system, so the shares are one
    // answer computed in pieces rather than several answers to reconcile, and
    // nothing passes between them while they run. A solved quantity cannot cross
    // a thread boundary, so each worker describes its own result on the way out.
    //
    // They go to the shared pool rather than to threads of their own, so that a
    // caller already spreading work across it — weighing several proposals, say —
    // divides one machine between them instead of multiplying its cores.
    let solved: Vec<Result<transfer::Sent, EvaluationError>> = shares
        .par_iter()
        .enumerate()
        .map(|(index, share)| {
            let evaluation = horizon(
                model,
                catalogue,
                mutators,
                overrides,
                *share,
                reporting.sharing(index, shares.len()),
            )?;
            transfer::sent(&evaluation).map_err(|error| EvaluationError::Evaluation {
                location: "a solved share".to_owned(),
                message: error.to_string(),
            })
        })
        .collect();
    let solved = shares
        .iter()
        .zip(solved)
        .map(|(share, sent)| {
            Ok(merge::Share {
                width: share.ensemble().len(),
                evaluation: transfer::restored(sent?),
            })
        })
        .collect::<Result<Vec<_>, EvaluationError>>()?;
    Ok(merge::merge(solved))
}

/// Advances one share of the draws across the whole horizon.
fn horizon(
    model: &SystemModel,
    catalogue: &BTreeMap<String, ComponentType>,
    mutators: &BTreeMap<String, Mutator>,
    overrides: &BTreeMap<String, String>,
    config: EvaluationConfig,
    reporting: Reporting<'_>,
) -> Result<Evaluation, EvaluationError> {
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let mut previous: BTreeMap<ComponentId, ComponentState> = BTreeMap::new();
    let mut carried: BTreeMap<LinkId, LinkState> = BTreeMap::new();
    let steps = config.horizon.max(1);
    let mut solved = Vec::with_capacity(steps);
    for index in 0..steps {
        let time = index as f64 * config.step;
        // Shared quantities may depend on the elapsed time, so the plan is
        // resolved afresh at each step and held fixed while it relaxes.
        let plan = time!(
            Prepare,
            prepare(
                model,
                catalogue,
                mutators,
                overrides,
                Timing {
                    seed: config.seed,
                    ensemble: config.ensemble(),
                    time,
                    step: config.step,
                },
            )
        )?;
            let step = time!(
                Relax,
                relax(
                &plan,
                &previous,
                &carried,
                time,
                config,
                &mut rng,
                reporting.at(index, steps),
                )
            )?;
        previous.clone_from(&step.components);
        carried.clone_from(&step.links);
        solved.push(step);
    }
    Ok(Evaluation { steps: solved })
}

pub(super) fn builtin_mutators_or_empty() -> BTreeMap<String, Mutator> {
    crate::system::catalogue::builtin_mutators().unwrap_or_default()
}
