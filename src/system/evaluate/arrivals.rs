//! Gathering what arrives at a component's ports from the wires attached to them.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::{Runtime, Value},
    system::{
        compile::{Plan, PreparedComponent},
        model::ComponentId,
    },
};

use super::{
    aggregate::{blank, combine},
    config::{EvaluationConfig, SolveMode},
    error::EvaluationError,
    flow::{CAPACITY, Direction, LATENCY, RATE, SUCCESS, sum, survives},
    mutate::apply,
    queue::queued,
    state::{ComponentState, LinkId, LinkState},
};

/// Collects the flows arriving on each of a component's ports, one direction.
///
/// Requests are gathered on inbound ports from the callers attached to them;
/// responses are gathered on outbound ports from the dependencies attached to
/// them. Both pass through the behaviours on the relationship before being
/// counted, so a retry policy's amplification and a timeout's cap are already
/// reflected in what the component reads.
///
/// Arrivals combine as their signal declares: rates add, latency takes the
/// largest, success multiplies, and per-operation quantities average. Extensive
/// signals are then divided by the component's share, so a component inside a
/// sharded scale unit reads the demand reaching one replica rather than the
/// whole fleet's. That is what makes a constraint answer "does one cell have
/// enough capacity", which is the question an engineer can act on.
///
/// Only signals that travel this way are present, each defaulting to its resting
/// value, so a component at the edge of a model reads no demand rather than
/// failing on a missing key, and can never read back the figures it publishes
/// itself.
#[allow(clippy::too_many_arguments)]
pub(super) fn arrivals(
    plan: &Plan,
    component: &PreparedComponent,
    current: &BTreeMap<ComponentId, ComponentState>,
    config: EvaluationConfig,
    time: f64,
    direction: Direction,
    links: &mut BTreeMap<LinkId, LinkState>,
    runtime: &mut Runtime,
) -> Result<BTreeMap<String, BTreeMap<String, Value>>, EvaluationError> {
    let ports = match direction {
        Direction::Request => &component.inbound,
        Direction::Response => &component.outbound,
    };
    let own = current.get(&component.id);
    let mut gathered = BTreeMap::new();
    for (name, port) in ports {
        // Whatever this component publishes onto the port, before the wire has
        // had its say. The flows going the other way are what the mutators read,
        // and they read them as the caller would see them, so the queue below is
        // applied to this too.
        let mine = {
            let mut values = blank(plan);
            let published = own.and_then(|state| match direction {
                Direction::Request => state.responses.get(name),
                Direction::Response => state.requests.get(name),
            });
            values.extend(published.cloned().unwrap_or_default());
            values
        };
        let mut collected: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for link in &port.links {
            let published = current.get(&link.peer).and_then(|state| match direction {
                Direction::Request => state.requests.get(&link.peer_port),
                Direction::Response => state.responses.get(&link.peer_port),
            });
            let Some(published) = published else {
                continue;
            };
            let mut flow = blank(plan);
            flow.extend(published.clone());
            let mut counterpart = mine.clone();

            // The wire itself is a queue, and its cost lands on the caller: work
            // waits in front of the callee, and once the buffer is full the
            // excess is refused. Both show up in the response, as delay and as
            // failure.
            //
            // The response is rewritten whichever way this pass is gathering,
            // because both ends have to agree about it. Read from the caller it
            // is the answer arriving; read from the callee it is what the
            // behaviours on the wire are reacting to, and a retry policy that
            // saw the callee's unqueued success would never learn that the wire
            // in front of it was turning its requests away.
            //
            // The rate travelling the other way is left alone deliberately. It
            // is what was asked for, not what got through, and a component that
            // saw only what got through could never report being over its
            // capacity — the wire would have trimmed the demand to fit before it
            // arrived, and the one figure that says how badly a design is
            // undersized would always read exactly one.
            let (request, response) = match direction {
                Direction::Request => (&flow, &counterpart),
                Direction::Response => (&counterpart, &flow),
            };
            // What crosses the wire is what the behaviours on it produce, not
            // what the caller first offered. A retry policy reissuing a failed
            // call sends that call again, and the buffer in front of the callee
            // holds every one of those attempts. Measuring the queue against the
            // caller's original rate would let a retry storm be invisible to the
            // very queue it fills.
            //
            // Those behaviours are shown the wire as it stood on the last pass,
            // because what they do depends on how full it is and how full it is
            // depends on what they do. Relaxation is what resolves that: each
            // pass answers with the previous pass's queue and the two converge
            // together, which is the same treatment every other loop in the
            // model gets.
            let mut observed = response.clone();
            if let Some(before) = links.get(&link.id) {
                if let Some(latency) = observed.get_mut(LATENCY) {
                    *latency = sum(latency, &before.wait, config);
                }
                if let Some(success) = observed.get_mut(SUCCESS) {
                    *success = survives(success, &before.blocked, config);
                }
            }
            let mut crossing = request.clone();
            for mutator in &link.mutators {
                crossing = apply(
                    plan,
                    mutator,
                    crossing,
                    &observed,
                    Direction::Request,
                    config,
                    time,
                    runtime,
                )?;
            }
            let state = match config.mode {
                // Balance: the backlog is whatever the current load implies, so
                // it is recomputed as the load settles.
                SolveMode::Steady => queued(&crossing, response, &link.capacity, config),
                // Time: the backlog was fixed when the step began. Only the
                // flows are recorded, so the next step has something to advance
                // from once everything else has settled.
                SolveMode::Transient => {
                    let mut carried = links.get(&link.id).cloned().unwrap_or_default();
                    carried.offered = crossing.get(RATE).cloned().unwrap_or(Value::Number(0.0));
                    carried.drain = response
                        .get(CAPACITY)
                        .cloned()
                        .unwrap_or(Value::Number(0.0));
                    carried
                }
            };
            let queueing = match direction {
                Direction::Request => &mut counterpart,
                Direction::Response => &mut flow,
            };
            if let Some(latency) = queueing.get_mut(LATENCY) {
                *latency = sum(latency, &state.wait, config);
            }
            if let Some(success) = queueing.get_mut(SUCCESS) {
                *success = survives(success, &state.blocked, config);
            }
            links.insert(link.id.clone(), state);
            // Behaviours are declared in the order a request meets them, so a
            // response meets them in the opposite order. A timeout written
            // beneath a retry has to convert slowness into failure before the
            // retry above it decides whether there is anything to reissue;
            // applying them in declaration order both ways would let the retry
            // answer a question the timeout had not yet asked, and the design
            // would look as though its deadline cost nothing.
            let ordered: Vec<_> = match direction {
                Direction::Request => link.mutators.iter().collect(),
                Direction::Response => link.mutators.iter().rev().collect(),
            };
            for mutator in ordered {
                flow = apply(
                    plan,
                    mutator,
                    flow,
                    &counterpart,
                    direction,
                    config,
                    time,
                    runtime,
                )?;
            }
            for (signal, value) in flow {
                collected.entry(signal).or_default().push(value);
            }
        }

        let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
        let mut combined = BTreeMap::new();
        for (signal, declaration) in &plan.signals {
            let values = collected.remove(signal).unwrap_or_default();
            let divisor = if declaration.extensive {
                component.share
            } else {
                1.0
            };
            combined.insert(
                signal.clone(),
                combine(&values, declaration.aggregate, divisor, config, &mut rng),
            );
        }
        gathered.insert(name.clone(), combined);
    }
    Ok(gathered)
}
