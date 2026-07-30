//! Carrying a solved share back from the thread that solved it.
//!
//! A share is an independent solve over its own draws, so the shares run at
//! once. What stops the result coming straight back is that a solved quantity is
//! a [`Value`], and a `Value` can hold a callable that closes over the scope it
//! was defined in — see [`crate::squiggle::snapshot`]. So a worker describes its
//! own result in the transferable form on the way out, and the thread that asked
//! for it reads that back.
//!
//! Only the quantities need describing. Everything else a step reports is
//! already plain data, and the sharing between two references to one binding
//! survives the crossing because a snapshot clones the handle rather than the
//! draws.

use std::collections::BTreeMap;

use crate::{
    squiggle::{
        Value,
        snapshot::{Snapshot, SnapshotError, Transferred},
    },
    system::model::ComponentId,
};

use super::state::{ComponentState, Evaluation, LinkId, LinkState, Mixture, Step, Unsettled};

/// Per-port, per-signal quantities, in the form that crosses.
type SentPorts = BTreeMap<String, BTreeMap<String, Transferred>>;

/// One share's solved horizon, ready to leave the thread that solved it.
pub(super) struct Sent {
    steps: Vec<SentStep>,
}

struct SentStep {
    time: f64,
    components: BTreeMap<ComponentId, SentComponent>,
    links: BTreeMap<LinkId, SentLink>,
    converged: bool,
    unsettled: Option<Unsettled>,
    mixture: Option<Mixture>,
    iterations: usize,
    movement: f64,
}

struct SentComponent {
    channels: BTreeMap<String, Transferred>,
    requests: SentPorts,
    responses: SentPorts,
    arriving: SentPorts,
    returning: SentPorts,
}

struct SentLink {
    backlog: Transferred,
    wait: Transferred,
    blocked: Transferred,
    offered: Transferred,
    drain: Transferred,
    transfer: Transferred,
    bandwidth: Transferred,
}

/// Describes a solved share in the form another thread can receive.
///
/// # Errors
///
/// Returns [`SnapshotError`] where a channel resolved to a callable, which a
/// solved quantity never is.
pub(super) fn sent(evaluation: &Evaluation) -> Result<Sent, SnapshotError> {
    let steps = evaluation
        .steps
        .iter()
        .map(|step| {
            Ok(SentStep {
                time: step.time,
                components: step
                    .components
                    .iter()
                    .map(|(id, state)| Ok((id.clone(), component(state)?)))
                    .collect::<Result<_, SnapshotError>>()?,
                links: step
                    .links
                    .iter()
                    .map(|(id, state)| Ok((id.clone(), link(state)?)))
                    .collect::<Result<_, SnapshotError>>()?,
                converged: step.converged,
                unsettled: step.unsettled.clone(),
                mixture: step.mixture.clone(),
                iterations: step.iterations,
                movement: step.movement,
            })
        })
        .collect::<Result<_, SnapshotError>>()?;
    Ok(Sent { steps })
}

/// Rebuilds a share's result on the thread that received it.
pub(super) fn restored(sent: Sent) -> Evaluation {
    let steps = sent
        .steps
        .into_iter()
        .map(|step| Step {
            time: step.time,
            components: step
                .components
                .into_iter()
                .map(|(id, state)| {
                    let state = ComponentState {
                        channels: quantities(state.channels),
                        requests: ports(state.requests),
                        responses: ports(state.responses),
                        arriving: ports(state.arriving),
                        returning: ports(state.returning),
                    };
                    (id, state)
                })
                .collect(),
            links: step
                .links
                .into_iter()
                .map(|(id, state)| {
                    let state = LinkState {
                        backlog: Value::restore(state.backlog),
                        wait: Value::restore(state.wait),
                        blocked: Value::restore(state.blocked),
                        offered: Value::restore(state.offered),
                        drain: Value::restore(state.drain),
                        transfer: Value::restore(state.transfer),
                        bandwidth: Value::restore(state.bandwidth),
                    };
                    (id, state)
                })
                .collect(),
            converged: step.converged,
            unsettled: step.unsettled,
            mixture: step.mixture,
            iterations: step.iterations,
            movement: step.movement,
        })
        .collect();
    Evaluation { steps }
}

fn component(state: &ComponentState) -> Result<SentComponent, SnapshotError> {
    Ok(SentComponent {
        channels: sent_quantities(&state.channels)?,
        requests: sent_ports(&state.requests)?,
        responses: sent_ports(&state.responses)?,
        arriving: sent_ports(&state.arriving)?,
        returning: sent_ports(&state.returning)?,
    })
}

fn link(state: &LinkState) -> Result<SentLink, SnapshotError> {
    Ok(SentLink {
        backlog: state.backlog.snapshot()?,
        wait: state.wait.snapshot()?,
        blocked: state.blocked.snapshot()?,
        offered: state.offered.snapshot()?,
        drain: state.drain.snapshot()?,
        transfer: state.transfer.snapshot()?,
        bandwidth: state.bandwidth.snapshot()?,
    })
}

fn sent_quantities(
    values: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Transferred>, SnapshotError> {
    values
        .iter()
        .map(|(name, value)| Ok((name.clone(), value.snapshot()?)))
        .collect()
}

fn sent_ports(
    values: &BTreeMap<String, BTreeMap<String, Value>>,
) -> Result<SentPorts, SnapshotError> {
    values
        .iter()
        .map(|(port, signals)| Ok((port.clone(), sent_quantities(signals)?)))
        .collect()
}

fn quantities(values: BTreeMap<String, Transferred>) -> BTreeMap<String, Value> {
    values
        .into_iter()
        .map(|(name, value)| (name, Value::restore(value)))
        .collect()
}

fn ports(values: SentPorts) -> BTreeMap<String, BTreeMap<String, Value>> {
    values
        .into_iter()
        .map(|(port, signals)| (port, quantities(signals)))
        .collect()
}
