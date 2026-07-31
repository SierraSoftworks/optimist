//! The worked example under `examples/queued-collapse`, checked end to end.
//!
//! The example exists to show what a second state variable does to a design: a
//! queue holds what a socket buffer cannot, and what it holds it keeps, so the
//! episode outlasts its cause and the design has two ways to be at one level of
//! demand. These tests guard those conclusions rather than the exact numbers.
//!
//! Showing that an episode outlasts its cause means solving well past the cause,
//! so this is a `comprehensive_tests` binary.
#![cfg(feature = "comprehensive_tests")]

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{
        ComponentState, EvaluationConfig, InterventionId, LoadedSystem, Solve, SolveMode,
        read_system,
    },
};

fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/queued-collapse")
}

fn loaded() -> LoadedSystem {
    read_system(&design()).expect("reads")
}

/// Advancing through time is the whole point here, so every reading is
/// transient. The step is short against the time the queue takes to drain.
fn config(seconds: f64) -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 120,
        horizon: (seconds / 0.5) as usize + 1,
        step: 0.5,
        mode: SolveMode::Transient,
        ..EvaluationConfig::default()
    }
}

fn scalar(state: &ComponentState, channel: &str) -> f64 {
    match state
        .channels
        .get(channel)
        .unwrap_or_else(|| panic!("no channel '{channel}'"))
    {
        Value::Number(number) => *number,
        Value::Distribution(distribution) => distribution.mean().expect("a mean"),
        other => panic!("channel '{channel}' is not numeric: {other:?}"),
    }
}

fn at(seconds: f64, intervention: Option<&str>) -> BTreeMap<String, ComponentState> {
    let system = loaded();
    let config = config(seconds);
    let evaluation = {
        let asking = Solve::new(&system.model, &system.component_types)
            .mutators(&system.mutators)
            .with(config);
        match intervention {
            Some(id) => asking.intervention(&InterventionId::new(id)).evaluate(),
            None => asking.evaluate(),
        }
    }
    .expect("solves");
    evaluation
        .settled()
        .components
        .iter()
        .map(|(id, state)| (id.to_string(), state.clone()))
        .collect()
}

/// The example loads and describes the system it says it does.
#[test]
fn the_example_solves() {
    let system = loaded();
    assert_eq!(system.name, "Queued collapse");
    assert_eq!(system.model.components.len(), 5);
    assert!(
        system
            .model
            .components
            .iter()
            .any(|component| component.component_type.as_str() == "queue"),
        "the queue is what makes this design second order"
    );
}

/// The surge builds a backlog the queue cannot work off while it lasts.
#[test]
fn the_surge_builds_a_backlog() {
    assert!(scalar(&at(2.0, None)["jobs"], "backlog") < 1.0);
    let peak = scalar(&at(14.0, None)["jobs"], "backlog");
    assert!(
        peak > 1_000.0,
        "expected the surge to build a backlog, got {peak}"
    );
}

/// Working the backlog off takes far longer than building it did.
///
/// This is what a queue buys and what it costs. The surge lasts ten seconds;
/// the backlog it leaves is still being worked off long afterwards, because the
/// rate it drains at is set by the consumer rather than by the demand that has
/// already gone away.
#[test]
fn recovery_outlasts_the_cause() {
    let peak = scalar(&at(16.0, None)["jobs"], "backlog");
    let later = scalar(&at(40.0, None)["jobs"], "backlog");
    assert!(
        later > 0.0,
        "expected the backlog to still be draining twenty five seconds later"
    );
    assert!(
        later < peak,
        "expected the backlog to be falling, got {peak} then {later}"
    );
    // A ten second surge, and the queue is still not empty four times later.
    assert!(
        scalar(&at(60.0, None)["jobs"], "backlog") > 0.0,
        "expected recovery to outlast the cause several times over"
    );
}

/// The design has two steady states at one level of demand.
///
/// Both readings are taken at the same moment, at the same offered load, with
/// the queue empty in each. They differ only in what happened earlier, and that
/// is enough to leave one of them an order of magnitude slower than the other.
/// Nothing in the model asserts this; it emerges from draining being load.
#[test]
fn the_collapsed_state_outlives_the_backlog() {
    let after = at(140.0, None);
    let never = at(140.0, Some("no-surge"));

    // Averaged over draws, so "empty" is a small number rather than exactly
    // zero. What matters is that there is no backlog left to explain the
    // difference below.
    let residual = scalar(&after["jobs"], "backlog");
    assert!(
        residual < 1.0,
        "the queue must have emptied for this to be a fair comparison, got {residual}"
    );
    assert!(scalar(&never["jobs"], "backlog") < 1.0);

    let collapsed = scalar(&after["shoppers"], "latency");
    let healthy = scalar(&never["shoppers"], "latency");
    assert!(
        collapsed > healthy * 10.0,
        "expected two distinct steady states, got {collapsed} against {healthy}"
    );
}

/// Refusing work at the edge is the only lever that prevents the episode.
///
/// It has to be set from what the consumer drains at rather than from what the
/// front end could serve, which is why it is the one intervention here that
/// works and the one nobody sizes correctly.
#[test]
fn shedding_at_the_edge_prevents_the_collapse() {
    let shed = at(140.0, Some("shed"));
    let never = at(140.0, Some("no-surge"));
    let shed_latency = scalar(&shed["shoppers"], "latency");
    let healthy = scalar(&never["shoppers"], "latency");
    assert!(
        (shed_latency - healthy).abs() < healthy,
        "expected shedding to leave the design healthy, got {shed_latency} against {healthy}"
    );
}

/// Shedding is not free, and the model says so.
///
/// A caller turned away at the door has not been served. A design that reported
/// a shedding system as perfectly successful would make shedding look like a
/// free win rather than a choice about who to disappoint.
#[test]
fn shedding_is_charged_for() {
    let during = scalar(&at(12.0, Some("shed"))["shoppers"], "success");
    assert!(
        during < 0.9,
        "expected refused requests to be counted as failures, got {during}"
    );
}
