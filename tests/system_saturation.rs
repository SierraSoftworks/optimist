//! The worked example under `examples/saturation`, checked end to end.
//!
//! The example exists to show where saturation comes from and what a retry
//! policy does to a design that has reached it. These tests guard those
//! conclusions rather than the exact numbers: the example is documentation, and
//! a change that stops it teaching what it claims is a defect in the example or
//! in the engine.

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{ComponentState, EvaluationConfig, InterventionId, LoadedSystem, Solve, read_system},
};

fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/saturation")
}

fn loaded() -> LoadedSystem {
    read_system(&design()).expect("reads")
}

fn config(horizon: usize) -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 200,
        horizon,
        step: 1.0,
        ..EvaluationConfig::default()
    }
}

/// Reads a channel as a single number, whether or not it carries uncertainty.
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

/// Solves the design at a moment, optionally with an intervention applied.
fn at(seconds: usize, intervention: Option<&str>) -> BTreeMap<String, ComponentState> {
    let system = loaded();
    let config = config(seconds + 1);
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

/// The example loads, and describes the system it says it does.
#[test]
fn the_example_solves() {
    let system = loaded();
    assert_eq!(system.name, "Saturation and retries");
    assert_eq!(system.model.components.len(), 3);
    assert!(
        system
            .model
            .interventions
            .iter()
            .any(|intervention| intervention.id.as_str() == "no-retries"),
        "the retry comparison is the example's headline"
    );
}

/// At rest the design is comfortable, and the store answers at its idle latency.
///
/// This is the reading a capacity plan is signed off against, and everything
/// the example goes on to show is a departure from it.
#[test]
fn at_rest_the_design_is_healthy() {
    let solved = at(0, None);
    assert!(
        scalar(&solved["browsers"], "success") > 0.99,
        "expected a healthy resting state, got {}",
        scalar(&solved["browsers"], "success")
    );
    // Within a factor of two of the idle figure a benchmark would report.
    let latency = scalar(&solved["orders"], "latency");
    assert!(
        latency < 0.02,
        "expected the store near its idle latency, got {latency}"
    );
}

/// Saturation is a property of load meeting a concurrency limit, not of the
/// store being slow.
///
/// The store's own service time is unchanged between these two readings. What
/// changes is how much is in flight against its pool, and that alone is enough
/// to move its answer time by two orders of magnitude.
#[test]
fn demand_alone_saturates_the_store() {
    let rest = scalar(&at(0, None)["orders"], "latency");
    let surge = scalar(&at(9, None)["orders"], "latency");
    assert!(
        surge > rest * 20.0,
        "expected saturation to dominate, got {rest} at rest and {surge} under load"
    );
}

/// Retrying a saturated dependency lowers the share of requests that succeed.
///
/// This is the example's point and the least intuitive thing in it. The retry
/// policy is answering failures that its own amplification is causing, so
/// removing it improves the very number it was added to protect.
#[test]
fn retrying_lowers_success_under_saturation() {
    let with_retries = scalar(&at(9, None)["browsers"], "success");
    let without = scalar(&at(9, Some("no-retries"))["browsers"], "success");
    assert!(
        without > with_retries,
        "expected retries to cost success, got {with_retries} with and {without} without"
    );
    assert!(
        without - with_retries > 0.1,
        "expected the cost to be substantial, got {with_retries} against {without}"
    );
}

/// A deadline set well clear of the fold removes the amplification entirely.
///
/// The retry policy is unchanged and still attached. What changes is that the
/// timeout stops calling a congested-but-working store a failure, so there is
/// nothing for the policy to answer.
#[test]
fn a_patient_deadline_removes_the_amplification() {
    let tight = scalar(&at(9, None)["browsers"], "success");
    let patient = scalar(&at(9, Some("patient-timeout"))["browsers"], "success");
    assert!(
        patient > tight,
        "expected a patient deadline to help, got {tight} tight and {patient} patient"
    );
}

/// A first-order design recovers the moment demand does.
///
/// The wire in front of the API holds a few hundred requests and drains against
/// a surplus of thousands per second, so there is nothing left of the surge one
/// step after it ends. This is the property the queued example does not have,
/// and asserting it here is what makes the contrast between the two meaningful.
#[test]
fn recovery_is_immediate_once_the_surge_passes() {
    let after = at(16, None);
    let never = at(16, Some("no-surge"));
    let recovered = scalar(&after["browsers"], "success");
    let untouched = scalar(&never["browsers"], "success");
    assert!(
        (recovered - untouched).abs() < 0.01,
        "expected full recovery, got {recovered} against {untouched} for a design with no surge"
    );
}
