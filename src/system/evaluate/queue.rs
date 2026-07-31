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
    system::values::{Varying, aligned, all_uniform, from_draws},
};

use super::{
    config::EvaluationConfig,
    flow::{CAPACITY, PAYLOAD, RATE},
    state::LinkState,
};

/// What a wire does to the flow crossing it, beyond holding it in a queue.
///
/// A link with a stated speed is a server as well as a buffer: an operation's
/// bytes take time to put on it, and only so many bytes fit through it each
/// second. Both fall out of the same two figures — how fast the link is and how
/// large an operation is — as does the byte rate the link is measured against,
/// so all three are taken in one pass over the draws.
pub(super) struct Carriage {
    /// Bytes per second crossing the wire, request and reply together.
    pub(super) transfer: Value,
    /// Operations per second the wire's speed allows at this payload.
    pub(super) throughput: Value,
    /// Seconds a round trip costs on an idle wire.
    pub(super) transit: Value,
}

impl Default for Carriage {
    /// A wire with nothing measurable on it: no bytes, no limit, no delay.
    fn default() -> Self {
        Self {
            transfer: Value::Number(0.0),
            throughput: Value::Number(f64::INFINITY),
            transit: Value::Number(0.0),
        }
    }
}

/// Measures one wire against the flow crossing it and the distance it spans.
///
/// One reply per request, so both payloads travel at the same operation rate and
/// add. Reading the two together is what makes batching legible: dividing the
/// rate while multiplying the payload leaves the byte rate exactly where it was,
/// which is the whole of the trade and the reason it is the right figure against
/// an operation limit and the wrong one against a link speed.
///
/// The delay is the round trip plus the time to put those bytes on the wire,
/// which is what an idle link costs. What a busy one costs on top of that is the
/// queue, and the queue is solved from [`throughput`](Carriage::throughput)
/// rather than charged here.
pub(super) fn carriage(
    request: &BTreeMap<String, Value>,
    response: &BTreeMap<String, Value>,
    bandwidth: &Value,
    latency: &Value,
    config: EvaluationConfig,
) -> Carriage {
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
    let speed = Varying::of(bandwidth, config.ensemble(), &mut rng);
    let distance = Varying::of(latency, config.ensemble(), &mut rng);
    let (Some(rate), Some(sent), Some(received), Some(speed), Some(distance)) =
        (rate, sent, received, speed, distance)
    else {
        return Carriage::default();
    };
    let columns = [rate, sent, received, speed, distance];
    let [transfer, throughput, transit] =
        solved(&columns, count, |[rate, sent, received, speed, distance]| {
            let size = sent.max(0.0) + received.max(0.0);
            // A speed nobody stated, and a flow carrying nothing, both leave the
            // wire a pure operation queue. So does a speed of zero, which is not
            // a link that carries nothing but an author who meant to say nothing.
            let (throughput, serialisation) = if speed > 0.0 && speed.is_finite() && size > 0.0 {
                (speed / size, size / speed)
            } else {
                (f64::INFINITY, 0.0)
            };
            [
                rate.max(0.0) * size,
                throughput,
                distance.max(0.0) + serialisation,
            ]
        });
    Carriage {
        transfer,
        throughput,
        transit,
    }
}

/// Solves the queue on one wire for the load crossing it.
///
/// A relationship holds work that has been offered but not yet taken. How much
/// waits, and how much is refused outright, follows from the ratio of what
/// arrives to what can be taken away, against how deep the wire is. The bounded
/// results are used rather than the unbounded ones because a real buffer fills
/// and then refuses: reporting an ever-growing delay for a queue that cannot
/// grow would overstate latency and understate failure at exactly the moment
/// both matter.
///
/// What can be taken away is the slower of the two things in the way: the far
/// end's capacity, and the operation rate the wire's own speed allows. A link
/// carrying more bytes than it can move backs up in front of a dependency with
/// cores to spare, and it is the wire that has to say so because neither end
/// can see it.
///
/// This is the steady-state solution, so the backlog reported is the one that
/// balances at the current load rather than one integrated over time.
pub(super) fn queued(
    request: &BTreeMap<String, Value>,
    response: &BTreeMap<String, Value>,
    capacity: &Value,
    throughput: &Value,
    config: EvaluationConfig,
) -> LinkState {
    let count = config.ensemble().len();
    let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
    let offered = request
        .get(RATE)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let taken = response
        .get(CAPACITY)
        .and_then(|value| Varying::of(value, config.ensemble(), &mut rng));
    let depth = Varying::of(capacity, config.ensemble(), &mut rng);
    let ceiling = Varying::of(throughput, config.ensemble(), &mut rng);
    let (Some(offered), Some(taken), Some(depth), Some(ceiling)) = (offered, taken, depth, ceiling)
    else {
        return LinkState::default();
    };

    let columns = [offered, taken, depth, ceiling];
    let [backlog, wait, blocked, drain] =
        solved(&columns, count, |[offered, taken, depth, ceiling]| {
            let rate = offered.max(0.0);
            let served = taken.min(ceiling);
            let held = depth.max(0.0);
            // An unattached or unlimited dependency drains anything offered, so
            // nothing waits and nothing is refused.
            if !served.is_finite() || served <= 0.0 {
                return [0.0, 0.0, 0.0, served];
            }
            let utilisation = rate / served;
            let [length, blocked] = bounded(utilisation, held);
            [length, length / served, blocked, served]
        });
    LinkState {
        backlog,
        wait,
        blocked,
        offered: request.get(RATE).cloned().unwrap_or(Value::Number(0.0)),
        drain,
        ..LinkState::default()
    }
}

/// Builds the quantities a solved wire reports from one pass over its draws.
///
/// A wire whose inputs are all certain has a certain answer, and computing it once
/// rather than a thousand identical times is what keeps an unloaded relationship
/// from costing as much as a saturated one.
fn solved<const N: usize, const M: usize>(
    columns: &[Varying; N],
    count: usize,
    solve: impl Fn([f64; N]) -> [f64; M],
) -> [Value; M] {
    let row = |index: usize| solve(std::array::from_fn(|slot| columns[slot].at(index)));
    if all_uniform(columns) {
        return row(0).map(Value::Number);
    }
    let span = aligned(columns, count);
    let mut solved: [Vec<f64>; M] = std::array::from_fn(|_| Vec::with_capacity(span));
    for index in 0..span {
        for (into, value) in solved.iter_mut().zip(row(index)) {
            into.push(value);
        }
    }
    solved.map(|draws| {
        count!(Draws, draws.len());
        from_draws(draws).unwrap_or(Value::Number(0.0))
    })
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
    let [backlog, wait, blocked] = solved(&columns, count, |[held, offered, drain, depth]| {
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
        transit: before.transit.clone(),
        blocked,
        offered: before.offered.clone(),
        drain: before.drain.clone(),
        transfer: before.transfer.clone(),
        bandwidth: before.bandwidth.clone(),
    }
}

/// Mean number waiting in a buffer of `capacity` at this load, and the chance an
/// arrival finds it full.
///
/// The M/M/1/K results. Kept alongside the solver rather than reached through the
/// expression language because the wire is not something an author writes.
///
/// The two are returned together because they share $\rho^{K+1}$, which is the
/// expensive term in both and was being evaluated once for each.
fn bounded(utilisation: f64, capacity: f64) -> [f64; 2] {
    let held = capacity.max(0.0);
    let rho = utilisation.max(0.0);
    if (rho - 1.0).abs() < 1e-9 {
        return [
            capacity_or_zero(capacity, capacity / 2.0),
            1.0 / (capacity + 1.0),
        ];
    }
    let power = rho.powf(capacity + 1.0);
    if !power.is_finite() {
        return [
            capacity_or_zero(capacity, capacity),
            (1.0 - 1.0 / rho).clamp(0.0, 1.0),
        ];
    }
    let length = rho / (1.0 - rho) - (capacity + 1.0) * power / (1.0 - power);
    [
        capacity_or_zero(capacity, length.clamp(0.0, held)),
        ((1.0 - rho) * rho.powf(capacity) / (1.0 - power)).clamp(0.0, 1.0),
    ]
}

/// A buffer with no depth holds nothing, whatever the load.
fn capacity_or_zero(capacity: f64, length: f64) -> f64 {
    if capacity <= 0.0 { 0.0 } else { length }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn number(value: &Value) -> f64 {
        match value {
            Value::Number(number) => *number,
            other => panic!("not a certain quantity: {other:?}"),
        }
    }

    fn flow(pairs: [(&str, f64); 2]) -> BTreeMap<String, Value> {
        pairs
            .into_iter()
            .map(|(signal, value)| (signal.to_owned(), Value::Number(value)))
            .collect()
    }

    /// Distance is a round trip, and the bytes are paid once each way.
    #[rstest]
    // Nothing stated: an instant, unlimited wire.
    #[case(f64::INFINITY, 0.0, 0.0)]
    // 40 ms there and back, split evenly between the two directions.
    #[case(f64::INFINITY, 0.04, 0.04)]
    // 2 kB of request and 8 kB of reply over a 1 MB/s link.
    #[case(1.0e6, 0.0, 0.01)]
    // Both costs land on the same crossing.
    #[case(1.0e6, 0.04, 0.05)]
    fn a_crossing_costs_distance_and_bytes(
        #[case] bandwidth: f64,
        #[case] latency: f64,
        #[case] expected: f64,
    ) {
        let carried = carriage(
            &flow([(RATE, 10.0), (PAYLOAD, 2_000.0)]),
            &flow([(CAPACITY, 100.0), (PAYLOAD, 8_000.0)]),
            &Value::Number(bandwidth),
            &Value::Number(latency),
            EvaluationConfig::default(),
        );
        assert!(
            (number(&carried.transit) - expected).abs() < 1e-12,
            "transit was {}",
            number(&carried.transit)
        );
    }

    /// A link's speed is an operation rate once somebody says how big an
    /// operation is, and none at all until they do.
    #[rstest]
    #[case(f64::INFINITY, 2_000.0, 8_000.0, f64::INFINITY)]
    #[case(1.0e6, 0.0, 0.0, f64::INFINITY)]
    #[case(1.0e6, 2_000.0, 8_000.0, 100.0)]
    fn a_stated_speed_becomes_an_operation_ceiling(
        #[case] bandwidth: f64,
        #[case] request_size: f64,
        #[case] response_size: f64,
        #[case] expected: f64,
    ) {
        let carried = carriage(
            &flow([(RATE, 10.0), (PAYLOAD, request_size)]),
            &flow([(CAPACITY, 100.0), (PAYLOAD, response_size)]),
            &Value::Number(bandwidth),
            &Value::Number(0.0),
            EvaluationConfig::default(),
        );
        assert_eq!(number(&carried.throughput), expected);
    }

    /// The wire drains at the slower of the two things in the way, so a link too
    /// slow for its traffic backs up in front of a dependency with room to spare.
    #[rstest]
    #[case(f64::INFINITY, 1_000.0)]
    #[case(2_000.0, 1_000.0)]
    #[case(400.0, 400.0)]
    fn the_slower_of_link_and_dependency_drains_the_wire(
        #[case] throughput: f64,
        #[case] expected: f64,
    ) {
        let state = queued(
            &flow([(RATE, 500.0), (PAYLOAD, 0.0)]),
            &flow([(CAPACITY, 1_000.0), (PAYLOAD, 0.0)]),
            &Value::Number(100.0),
            &Value::Number(throughput),
            EvaluationConfig::default(),
        );
        assert_eq!(number(&state.drain), expected);
    }

    /// Offering more bytes than the link carries fills the buffer and turns the
    /// excess away, exactly as an overloaded dependency would.
    #[test]
    fn a_saturated_link_blocks_the_traffic_it_cannot_carry() {
        let state = queued(
            &flow([(RATE, 500.0), (PAYLOAD, 0.0)]),
            &flow([(CAPACITY, 1_000.0), (PAYLOAD, 0.0)]),
            &Value::Number(10.0),
            &Value::Number(100.0),
            EvaluationConfig::default(),
        );
        assert!(number(&state.blocked) > 0.5, "{}", number(&state.blocked));
        assert!(number(&state.wait) > 0.0);
    }
}
