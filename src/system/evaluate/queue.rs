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
    profile::count,
    squiggle::Value,
    system::values::{Varying, aligned, all_uniform, from_draws, per_draw},
};

use super::{
    config::EvaluationConfig,
    flow::{CAPACITY, PAYLOAD, RATE},
    state::LinkState,
};

/// Bytes per second crossing a wire, request and reply together.
///
/// One reply per request, so both payloads travel at the same operation rate and
/// add. Reading the two together is what makes batching legible: dividing the
/// rate while multiplying the payload leaves this figure exactly where it was,
/// which is the whole of the trade and the reason it is the right one against an
/// operation limit and the wrong one against a link speed.
pub(super) fn carried(
    request: &BTreeMap<String, Value>,
    response: &BTreeMap<String, Value>,
    config: EvaluationConfig,
) -> Value {
    let count = config.ensemble().len();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let rate = request
        .get(RATE)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let sent = request
        .get(PAYLOAD)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let received = response
        .get(PAYLOAD)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let (Some(rate), Some(sent), Some(received)) = (rate, sent, received) else {
        return Value::Number(0.0);
    };
    let columns = [rate, sent, received];
    let bytes = |index: usize| {
        columns[0].at(index).max(0.0)
            * (columns[1].at(index).max(0.0) + columns[2].at(index).max(0.0))
    };
    if all_uniform(&columns) {
        return Value::Number(bytes(0));
    }
    per_draw(aligned(&columns, count), bytes).unwrap_or(Value::Number(0.0))
}

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
    let count = config.ensemble().len();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let offered = request
        .get(RATE)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let drain = response
        .get(CAPACITY)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let depth = Varying::of(capacity, config.ensemble(), &mut rng);
    let (Some(offered), Some(drain), Some(depth)) = (offered, drain, depth) else {
        return LinkState::default();
    };

    let columns = [offered, drain, depth];
    let (backlog, wait, blocked) = solved(&columns, count, |[offered, drain, depth]| {
        let rate = offered.max(0.0);
        let served = drain;
        let held = depth.max(0.0);
        // An unattached or unlimited dependency drains anything offered, so
        // nothing waits and nothing is refused.
        if !served.is_finite() || served <= 0.0 {
            return [0.0; 3];
        }
        let utilisation = rate / served;
        let length = bounded_length(utilisation, held);
        [
            length,
            length / served,
            bounded_blocking(utilisation, held),
        ]
    });
    LinkState {
        backlog,
        wait,
        blocked,
        offered: request.get(RATE).cloned().unwrap_or(Value::Number(0.0)),
        drain: response
            .get(CAPACITY)
            .cloned()
            .unwrap_or(Value::Number(0.0)),
        ..LinkState::default()
    }
}

/// Builds the three quantities a solved wire reports from one pass over its draws.
///
/// A wire whose inputs are all certain has a certain answer, and computing it once
/// rather than a thousand identical times is what keeps an unloaded relationship
/// from costing as much as a saturated one.
fn solved<const N: usize>(
    columns: &[Varying; N],
    count: usize,
    solve: impl Fn([f64; N]) -> [f64; 3],
) -> (Value, Value, Value) {
    let row = |index: usize| solve(std::array::from_fn(|slot| columns[slot].at(index)));
    if all_uniform(columns) {
        let [backlog, wait, blocked] = row(0);
        return (
            Value::Number(backlog),
            Value::Number(wait),
            Value::Number(blocked),
        );
    }
    let span = aligned(columns, count);
    let mut solved = [
        Vec::with_capacity(span),
        Vec::with_capacity(span),
        Vec::with_capacity(span),
    ];
    for index in 0..span {
        for (into, value) in solved.iter_mut().zip(row(index)) {
            into.push(value);
        }
    }
    let [backlog, wait, blocked] = solved.map(|draws| {
        count!(Draws, draws.len());
        from_draws(draws).unwrap_or(Value::Number(0.0))
    });
    (backlog, wait, blocked)
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
    let count = config.ensemble().len();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let held = Varying::of(&before.backlog, config.ensemble(), &mut rng);
    let offered = Varying::of(&before.offered, config.ensemble(), &mut rng);
    let drain = Varying::of(&before.drain, config.ensemble(), &mut rng);
    let depth = Varying::of(capacity, config.ensemble(), &mut rng);
    let (Some(held), Some(offered), Some(drain), Some(depth)) = (held, offered, drain, depth)
    else {
        return before.clone();
    };

    let step = config.step.max(f64::EPSILON);
    let columns = [held, offered, drain, depth];
    let (backlog, wait, blocked) = solved(&columns, count, |[held, offered, drain, depth]| {
        let waiting = held.max(0.0);
        let rate = offered.max(0.0);
        let served = drain;
        let room = depth.max(0.0);
        if !served.is_finite() || served <= 0.0 {
            return [0.0; 3];
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
        [next, next / served, refused]
    });
    LinkState {
        backlog,
        wait,
        blocked,
        offered: before.offered.clone(),
        drain: before.drain.clone(),
        transfer: before.transfer.clone(),
        bandwidth: before.bandwidth.clone(),
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
