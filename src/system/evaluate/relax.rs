//! Relaxing one step of a model toward its fixed point.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::system::{
    compile::{Plan, runtime},
    model::ComponentId,
};

use super::{
    blend::converge,
    component::evaluate_component,
    config::{EvaluationConfig, SolveMode},
    error::EvaluationError,
    queue::advance,
    state::{ComponentState, LinkId, LinkState, Step},
};

/// Smallest step the adaptive damping will tighten to.
const MINIMUM_DAMPING: f64 = 0.02;

/// Contracting passes that must pass before a tightened step is relaxed again.
const RECOVERY_PASSES: usize = 8;

pub(super) fn relax(
    plan: &Plan,
    previous: &BTreeMap<ComponentId, ComponentState>,
    carried: &BTreeMap<LinkId, LinkState>,
    time: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> Result<Step, EvaluationError> {
    let mut current: BTreeMap<ComponentId, ComponentState> = plan
        .components
        .iter()
        .map(|component| {
            let state = previous.get(&component.id).cloned().unwrap_or_default();
            (component.id.clone(), state)
        })
        .collect();
    let mut links = seeded_links(plan, carried, config);
    let mut movement = f64::INFINITY;
    let mut iterations = 0;
    // Damping starts where the caller asked and tightens itself when the
    // iterate overshoots. A model carries a thousand draws at once and they do
    // not all sit in the same regime: most settle readily while a few, drawn
    // near a fold, cycle at a step size the rest converge happily at. Halving
    // on any pass that moved further than the one before it lets those few
    // settle without slowing the rest more than they need, and means the figure
    // in the configuration is a starting point rather than something an author
    // has to get right.
    //
    // The tightening is undone again once the iterate has contracted steadily
    // for a while, because `movement` is the worst draw anywhere in the model:
    // a single draw crossing a fold early on would otherwise hold every other
    // draw at a twentieth of the step for the rest of the solve, which costs an
    // order of magnitude in passes to reach the same fixed point.
    let ceiling = config.damping.clamp(f64::EPSILON, 1.0);
    let mut weight = ceiling;
    let mut before = f64::INFINITY;
    let mut contracting = 0_usize;
    // One runtime serves the whole relaxation. Building the standard
    // environment costs more than evaluating the short expressions a model is
    // made of, and a solve evaluates them thousands of times.
    let mut runtime = runtime(config.seed, config.sample_count)?;
    while iterations < config.max_iterations {
        iterations += 1;
        movement = 0.0;
        for component in &plan.components {
            let computed = evaluate_component(
                plan,
                component,
                &current,
                previous,
                time,
                config,
                &mut links,
                &mut runtime,
            )?;
            let settled = current
                .get(&component.id)
                .expect("every component was seeded above");
            let (blended, moved) = converge(component, settled, &computed, weight, config, rng);
            movement = movement.max(moved);
            current.insert(component.id.clone(), blended);
        }
        if movement <= config.tolerance {
            break;
        }
        if movement > before * 1.05 {
            weight = (weight * 0.5).max(MINIMUM_DAMPING);
            contracting = 0;
        } else {
            contracting += 1;
            if contracting >= RECOVERY_PASSES {
                weight = (weight * 2.0).min(ceiling);
                contracting = 0;
            }
        }
        before = movement;
    }
    Ok(Step {
        time,
        components: current,
        links,
        converged: movement <= config.tolerance,
        iterations,
        movement,
    })
}

/// Places every wire where the step begins.
///
/// Each wire starts the step where the last one left it. Solving for balance,
/// that is only a warm start and the relaxation will move it; advancing through
/// time, it is the state itself, integrated once here and then held still while
/// everything else settles around it. Holding it still is the point: a backlog
/// that changed underneath the relaxation would be being solved for rather than
/// carried, which is the loop that mode exists to cut.
fn seeded_links(
    plan: &Plan,
    carried: &BTreeMap<LinkId, LinkState>,
    config: EvaluationConfig,
) -> BTreeMap<LinkId, LinkState> {
    let mut links = carried.clone();
    if config.mode != SolveMode::Transient {
        return links;
    }
    for component in &plan.components {
        for port in component
            .inbound
            .values()
            .chain(component.outbound.values())
        {
            for link in &port.links {
                let before = links.get(&link.id).cloned().unwrap_or_default();
                links.insert(link.id.clone(), advance(&before, &link.capacity, config));
            }
        }
    }
    links
}
