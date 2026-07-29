//! Relaxing one step of a model toward its fixed point.

use std::collections::BTreeMap;

use rand_chacha::ChaCha20Rng;

use crate::{
    profile::{count, time},
    system::{
        compile::{Plan, runtime},
        model::ComponentId,
        values::Varying,
    },
};

use super::{
    blend::{Moved, converge},
    component::evaluate_component,
    config::{EvaluationConfig, SolveMode},
    damping::Damping,
    error::EvaluationError,
    modes::modes,
    progress::Reporting,
    queue::advance,
    retire,
    state::{ComponentState, LinkId, LinkState, Mixture, Step, Unsettled},
    stationary::drift,
};

/// Passes between checks that the iterate is still getting closer to something.
///
/// Long enough to sit out the adaptive damping's tighten-and-recover cycle and a
/// stretch of the slow crawl a loop gain just under one produces, so that a
/// design which is converging — merely without hurry — is never mistaken for one
/// that has stopped.
const PATIENCE: usize = 128;

/// Improvement over that span below which the iterate is treated as stuck.
///
/// A loop that is contracting at all beats this comfortably: even a ratio of
/// 0.999 per pass compounds to better than a tenth over the span. A loop with no
/// fixed point to find does not improve at all.
const PROGRESS: f64 = 0.98;

/// How a step stopped.
enum Outcome {
    /// Every draw reached a value it agrees with.
    Settled,
    /// The ensemble reached a distribution it agrees with while its draws went
    /// on swapping between the branches making it up.
    Mixed {
        /// Largest movement of any quantile over the window that proved it.
        drift: f64,
    },
    /// Something was still moving when the solver stopped.
    Moving {
        /// Whether the iterate had stopped closing, rather than run out of passes.
        stalled: bool,
    },
}

pub(super) fn relax(
    plan: &Plan,
    previous: &BTreeMap<ComponentId, ComponentState>,
    carried: &BTreeMap<LinkId, LinkState>,
    time: f64,
    mut config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
    reporting: Reporting<'_>,
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
    let draws = config.ensemble().len().max(1);
    let mut moved = vec![f64::INFINITY; draws];
    let mut movement = f64::INFINITY;
    let mut iterations = 0;
    let mut damping = Damping::opening(config.damping, draws);
    // One runtime serves the whole relaxation. Building the standard
    // environment costs more than evaluating the short expressions a model is
    // made of, and a solve evaluates them thousands of times.
    let mut runtime = runtime(config.seed, config.ensemble())?;
    // A design with no steady state to find will run to the iteration cap on
    // every step of every horizon, which is where nearly all of the time goes in
    // the one case where none of it buys anything. The iterate is watched for
    // whether it is still closing on something, and abandoned when it is not.
    let mut best = f64::INFINITY;
    let mut checkpoint = f64::INFINITY;
    let mut since_checkpoint = 0_usize;
    let mut outcome = Outcome::Moving { stalled: false };
    // The state this window opened on, kept so that the ensemble can be compared
    // with itself across a long span rather than between two adjacent passes,
    // where a design still moving slowly would look still.
    let mut opened = current.clone();
    let mut unsettled: Option<(ComponentId, Moved)> = None;
    let size = config.sample_count.max(1);
    let mut whole = current.clone();
    let mut whole_links = links.clone();
    while iterations < config.max_iterations {
        iterations += 1;
        count!(Passes);
        moved.fill(0.0);
        movement = 0.0;
        unsettled = None;
        let links_before = links.clone();
        for component in &plan.components {
            count!(Components);
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
            let (blended, furthest) = time!(
                Converge,
                converge(
                    component,
                    settled,
                    &computed,
                    &damping.stride(),
                    &mut moved,
                    config,
                    rng
                )
            );
            if furthest.distance >= movement || unsettled.is_none() {
                movement = furthest.distance;
                unsettled = Some((component.id.clone(), furthest));
            }
            current.insert(component.id.clone(), blended);
        }
        reporting.pass(
            iterations,
            config.max_iterations,
            movement,
            config.tolerance,
            unsettled.as_ref().and_then(|(component, moved)| {
                Some((component, moved.channel.as_deref()?))
            }),
        );
        if moved.iter().all(|draw| *draw <= config.tolerance) {
            outcome = Outcome::Settled;
            break;
        }
        damping.adapt(&moved);
        retire::stirred(&links_before, &links, config, rng, &mut moved);
        if let Some(spent) = retire::spent(&moved, config.share, size)
            && spent != 0
        {
            whole = retire::widen(&current, &whole, config.share, size, rng);
            whole_links = retire::widen_links(&links, &whole_links, config.share, size, rng);
            opened = retire::widen(&opened, &whole, config.share, size, rng);
            let live = config.share.retaining(config.share.live() & !spent);
            damping.retain(config.share, live, size);
            config.share = live;
            runtime = runtime.sharing(config.ensemble());
            current.clone_from(&whole);
            links.clone_from(&whole_links);
            moved = vec![0.0; config.ensemble().len()];
        }
        best = best.min(movement);
        since_checkpoint += 1;
        if since_checkpoint >= PATIENCE {
            if best > checkpoint * PROGRESS {
                // The draws have stopped closing. Whether the design has is a
                // different question, and this is the only place it is asked: an
                // ensemble that has been still for the whole window has found its
                // answer, and the draws underneath it are trading places between
                // branches rather than still searching for one.
                let settled = drift(&opened, &current, config, rng);
                outcome = if settled <= config.tolerance {
                    Outcome::Mixed { drift: settled }
                } else {
                    Outcome::Moving { stalled: true }
                };
                break;
            }
            checkpoint = best;
            since_checkpoint = 0;
            opened = current.clone();
        }
    }
    if config.share.live() != u64::MAX {
        current = retire::widen(&current, &whole, config.share, size, rng);
        links = retire::widen_links(&links, &whole_links, config.share, size, rng);
        config.share = config.share.retaining(u64::MAX);
    }
    let moving = unsettled.and_then(|(component, moved)| {
        moved.channel.map(|channel| (component, channel, moved.distance))
    });
    Ok(match outcome {
        Outcome::Settled => Step {
            time,
            components: current,
            links,
            converged: true,
            unsettled: None,
            mixture: None,
            iterations,
            movement,
        },
        Outcome::Mixed { drift } => {
            let mixture = moving.map(|(component, channel, swing)| Mixture {
                states: states(&current, &component, &channel, config, rng),
                component,
                channel,
                swing,
            });
            Step {
                time,
                components: current,
                links,
                converged: true,
                unsettled: None,
                mixture,
                iterations,
                movement: drift,
            }
        }
        Outcome::Moving { stalled } => Step {
            time,
            components: current,
            links,
            converged: false,
            unsettled: moving.map(|(component, channel, movement)| Unsettled {
                component,
                channel,
                movement,
                stalled,
            }),
            mixture: None,
            iterations,
            movement,
        },
    })
}

/// How many states one channel's draws divided between.
fn states(
    current: &BTreeMap<ComponentId, ComponentState>,
    component: &ComponentId,
    channel: &str,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> usize {
    current
        .get(component)
        .and_then(|state| state.channels.get(channel))
        .and_then(|value| Varying::of(value, config.ensemble(), rng))
        .and_then(|varying| varying.spread().map(|draws| modes(&draws)))
        .unwrap_or(1)
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
