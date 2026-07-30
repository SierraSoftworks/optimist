//! Gathering what arrives at a component's ports from the wires attached to them.

use std::collections::BTreeMap;

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;

use crate::{
    profile::time,
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
    flow::{CAPACITY, Direction, LATENCY, PEERS, RATE, SUCCESS, scaled, sum, survives},
    mutate::{apply, returning},
    queue::{carried, queued},
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
/// signals are scaled at each relationship boundary, so traffic stays local
/// inside one scale unit, divides when entering a sharded unit, and gathers when
/// leaving one. That is what makes a constraint answer "does one cell have
/// enough capacity" while a caller outside the cell sees the fleet as a whole.
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
        let mine = time!(Gather, {
            let mut values = blank(plan);
            let published = own.and_then(|state| match direction {
                Direction::Request => state.responses.get(name),
                Direction::Response => state.requests.get(name),
            });
            values.extend(published.cloned().unwrap_or_default());
            values
        });
        let mut collected: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for link in &port.links {
            let published = current.get(&link.peer).and_then(|state| match direction {
                Direction::Request => state.requests.get(&link.peer_port),
                Direction::Response => state.responses.get(&link.peer_port),
            });
            let Some(published) = published else {
                continue;
            };
            let (mut flow, mut counterpart) = time!(Gather, {
                let mut flow = blank(plan);
                flow.extend(published.clone());
                (flow, mine.clone())
            });
            let (flow_scale, counterpart_scale) = match direction {
                Direction::Request => (link.request_scale, link.response_scale),
                Direction::Response => (link.response_scale, link.request_scale),
            };
            scale_extensive(&mut flow, plan, flow_scale, config);
            scale_extensive(&mut counterpart, plan, counterpart_scale, config);

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
            let mut observed = time!(Gather, response.clone());
            if let Some(before) = links.get(&link.id) {
                if let Some(latency) = observed.get_mut(LATENCY) {
                    *latency = sum(latency, &before.wait, config);
                }
                if let Some(success) = observed.get_mut(SUCCESS) {
                    *success = survives(success, &before.blocked, config);
                }
            }
            // A behaviour reads the answer as it reaches it, which is the
            // callee's answer already rewritten by every behaviour beneath it.
            let returned = time!(
                Behaviours,
                returning(
                    plan,
                    &link.mutators,
                    observed,
                    request,
                    config,
                    time,
                    runtime,
                )
            )?;
            let mut crossing = time!(Gather, request.clone());
            time!(Behaviours, {
                for (mutator, observed) in link.mutators.iter().zip(&returned.views) {
                    crossing = apply(
                        plan,
                        mutator,
                        crossing,
                        observed,
                        Direction::Request,
                        config,
                        time,
                        runtime,
                    )?;
                }
                Ok::<(), EvaluationError>(())
            })?;
            let mut state = time!(
                Queue,
                match config.mode {
                    // Balance: the backlog is whatever the current load implies, so
                    // it is recomputed as the load settles.
                    SolveMode::Steady => queued(&crossing, response, &link.capacity, config),
                    // Time: the backlog was fixed when the step began. Only the
                    // flows are recorded, so the next step has something to advance
                    // from once everything else has settled.
                    SolveMode::Transient => {
                        let mut held = links.get(&link.id).cloned().unwrap_or_default();
                        held.offered = crossing.get(RATE).cloned().unwrap_or(Value::Number(0.0));
                        held.drain = response
                            .get(CAPACITY)
                            .cloned()
                            .unwrap_or(Value::Number(0.0));
                        held
                    }
                }
            );
            // Both ends compute this and agree, so it is taken from the request
            // as it crosses and the answer as it arrives back rather than from
            // whichever end happened to be evaluated last.
            state.transfer = time!(Queue, carried(&crossing, &returned.settled, config));
            state.bandwidth = link.bandwidth.clone();
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
            match direction {
                Direction::Request => {
                    // Rebuilt against the queue as it stands now rather than as
                    // it stood last pass, because this is the flow the component
                    // reads rather than the one the queue was solved from.
                    let offered = time!(Gather, flow.clone());
                    let returned = time!(
                        Behaviours,
                        returning(
                            plan,
                            &link.mutators,
                            counterpart.clone(),
                            &offered,
                            config,
                            time,
                            runtime,
                        )
                    )?;
                    time!(Behaviours, {
                        for (mutator, observed) in link.mutators.iter().zip(&returned.views) {
                            flow = apply(
                                plan,
                                mutator,
                                flow,
                                observed,
                                Direction::Request,
                                config,
                                time,
                                runtime,
                            )?;
                        }
                        Ok::<(), EvaluationError>(())
                    })?;
                }
                Direction::Response => {
                    time!(Behaviours, {
                        for mutator in link.mutators.iter().rev() {
                            flow = apply(
                                plan,
                                mutator,
                                flow,
                                &counterpart,
                                Direction::Response,
                                config,
                                time,
                                runtime,
                            )?;
                        }
                        Ok::<(), EvaluationError>(())
                    })?;
                }
            }
            let receive_scale = match direction {
                Direction::Request => link.request_receive_scale,
                Direction::Response => link.response_receive_scale,
            };
            scale_extensive(&mut flow, plan, receive_scale, config);
            time!(Gather, {
                // Topology rather than a published quantity: no component can
                // see how many replicas of its peer sit on the far end, so the
                // engine states it here, after the behaviours have had their
                // say, because a behaviour rewriting the shape of the deployment
                // is not something a relationship gets to do.
                flow.insert(PEERS.to_owned(), Value::Number(link.peers));
                for (signal, value) in flow {
                    collected.entry(signal).or_default().push(value);
                }
            });
        }

        let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
        let mut combined = BTreeMap::new();
        time!(Combine, {
            for (signal, declaration) in &plan.signals {
                let values = collected.remove(signal).unwrap_or_default();
                combined.insert(
                    signal.clone(),
                    combine(&values, declaration, 1.0, config, &mut rng),
                );
            }
        });
        gathered.insert(name.clone(), combined);
    }
    Ok(gathered)
}

fn scale_extensive(
    flow: &mut BTreeMap<String, Value>,
    plan: &Plan,
    factor: f64,
    config: EvaluationConfig,
) {
    if factor == 1.0 {
        return;
    }
    for (signal, value) in flow {
        if plan
            .signals
            .get(signal)
            .is_some_and(|declaration| declaration.extensive)
        {
            *value = scaled(value, factor, config);
        }
    }
}
