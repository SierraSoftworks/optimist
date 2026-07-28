//! The queue that sits on every relationship.
//!
//! A wire holds work that has been offered but not yet taken, and both of the
//! solve modes read that backlog from the same picture: a buffer of finite depth
//! fed at one rate and drained at another. What differs is whether the backlog
//! is solved for balance or carried forward a step at a time.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::values::{draws, from_draws},
};

use super::{
    config::EvaluationConfig,
    flow::{CAPACITY, RATE},
    state::LinkState,
};

/// Solves the queue on one wire for the load crossing it.
///
/// A relationship holds work that has been offered but not yet taken. How much
/// waits, and how much is refused outright, follows from the ratio of what
/// arrives to what the far end can drain, against how deep the wire is. The
/// bounded results are used rather than the unbounded ones because a real buffer
/// fills and then refuses: reporting an ever-growing delay for a queue that
/// cannot grow would overstate latency and understate failure at exactly the
/// moment both matter.
///
/// This is the steady-state solution, so the backlog reported is the one that
/// balances at the current load rather than one integrated over time.
pub(super) fn queued(
    request: &BTreeMap<String, Value>,
    response: &BTreeMap<String, Value>,
    capacity: &Value,
    config: EvaluationConfig,
) -> LinkState {
    let count = config.sample_count.max(1);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let offered = request
        .get(RATE)
        .and_then(|value| draws(value, count, &mut rng));
    let drain = response
        .get(CAPACITY)
        .and_then(|value| draws(value, count, &mut rng));
    let depth = draws(capacity, count, &mut rng);
    let (Some(offered), Some(drain), Some(depth)) = (offered, drain, depth) else {
        return LinkState::default();
    };

    let mut backlog = Vec::with_capacity(count);
    let mut wait = Vec::with_capacity(count);
    let mut blocked = Vec::with_capacity(count);
    for index in 0..count {
        let rate = offered[index].max(0.0);
        let served = drain[index];
        let held = depth[index].max(0.0);
        // An unattached or unlimited dependency drains anything offered, so
        // nothing waits and nothing is refused.
        if !served.is_finite() || served <= 0.0 {
            backlog.push(0.0);
            wait.push(0.0);
            blocked.push(0.0);
            continue;
        }
        let utilisation = rate / served;
        let length = bounded_length(utilisation, held);
        backlog.push(length);
        wait.push(length / served);
        blocked.push(bounded_blocking(utilisation, held));
    }
    LinkState {
        backlog: from_draws(backlog).unwrap_or(Value::Number(0.0)),
        wait: from_draws(wait).unwrap_or(Value::Number(0.0)),
        blocked: from_draws(blocked).unwrap_or(Value::Number(0.0)),
        offered: request.get(RATE).cloned().unwrap_or(Value::Number(0.0)),
        drain: response
            .get(CAPACITY)
            .cloned()
            .unwrap_or(Value::Number(0.0)),
    }
}

/// Advances one wire's backlog by a step, from the flows it last carried.
///
/// Forward Euler on the contents of a bounded buffer. What arrived last step and
/// what left it are both known, so the difference is what accumulated, and the
/// buffer's depth bounds the result at both ends: it cannot hold less than
/// nothing, and once full the excess is refused rather than stored.
///
/// The rates are the previous step's on purpose. Nothing about this step is
/// consulted, which is what makes the pass explicit and breaks the loop that
/// otherwise ties a queue's delay to the demand that delay is producing. It is
/// also what makes the step size matter: advance further than the queue takes to
/// drain and the integration will overshoot and oscillate, in the solver rather
/// than in the design.
pub(super) fn advance(before: &LinkState, capacity: &Value, config: EvaluationConfig) -> LinkState {
    let count = config.sample_count.max(1);
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let held = draws(&before.backlog, count, &mut rng);
    let offered = draws(&before.offered, count, &mut rng);
    let drain = draws(&before.drain, count, &mut rng);
    let depth = draws(capacity, count, &mut rng);
    let (Some(held), Some(offered), Some(drain), Some(depth)) = (held, offered, drain, depth)
    else {
        return before.clone();
    };

    let step = config.step.max(f64::EPSILON);
    let mut backlog = Vec::with_capacity(count);
    let mut wait = Vec::with_capacity(count);
    let mut blocked = Vec::with_capacity(count);
    for index in 0..count {
        let waiting = held[index].max(0.0);
        let rate = offered[index].max(0.0);
        let served = drain[index];
        let room = depth[index].max(0.0);
        if !served.is_finite() || served <= 0.0 {
            backlog.push(0.0);
            wait.push(0.0);
            blocked.push(0.0);
            continue;
        }
        // What the wire can take this step: whatever drains, plus whatever space
        // is left to store. Anything beyond that is turned away at the door.
        let admissible = served + (room - waiting).max(0.0) / step;
        let accepted = rate.min(admissible);
        let refused = if rate > 0.0 {
            ((rate - accepted) / rate).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let next = (waiting + (accepted - served) * step).clamp(0.0, room);
        backlog.push(next);
        wait.push(next / served);
        blocked.push(refused);
    }
    LinkState {
        backlog: from_draws(backlog).unwrap_or(Value::Number(0.0)),
        wait: from_draws(wait).unwrap_or(Value::Number(0.0)),
        blocked: from_draws(blocked).unwrap_or(Value::Number(0.0)),
        offered: before.offered.clone(),
        drain: before.drain.clone(),
    }
}

/// Mean number waiting in a buffer of `capacity` at this load.
///
/// The M/M/1/K result. Kept alongside the solver rather than reached through the
/// expression language because the wire is not something an author writes.
fn bounded_length(utilisation: f64, capacity: f64) -> f64 {
    if capacity <= 0.0 {
        return 0.0;
    }
    let rho = utilisation.max(0.0);
    if (rho - 1.0).abs() < 1e-9 {
        return capacity / 2.0;
    }
    let power = rho.powf(capacity + 1.0);
    if !power.is_finite() {
        return capacity;
    }
    let length = rho / (1.0 - rho) - (capacity + 1.0) * power / (1.0 - power);
    length.clamp(0.0, capacity)
}

/// Probability an arrival finds the buffer full and is refused.
fn bounded_blocking(utilisation: f64, capacity: f64) -> f64 {
    let rho = utilisation.max(0.0);
    if (rho - 1.0).abs() < 1e-9 {
        return 1.0 / (capacity + 1.0);
    }
    let power = rho.powf(capacity + 1.0);
    if !power.is_finite() {
        return (1.0 - 1.0 / rho).clamp(0.0, 1.0);
    }
    ((1.0 - rho) * rho.powf(capacity) / (1.0 - power)).clamp(0.0, 1.0)
}
