//! The worked example under `examples/deadlines`, checked end to end.
//!
//! The example exists to show that a timeout does two things which are usually
//! confused for one: it bounds what the caller waits for, and — only if the
//! cancellation travels — it withdraws the work nobody is waiting for any more.
//! These tests guard that conclusion, and with it the machinery the example
//! needs: a design defining its own behaviour, and that behaviour actually
//! reaching the solver.

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{
        ComponentState, EvaluationConfig, InterventionId, LoadedSystem, Solve, SolveMode,
        read_system,
    },
};

fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/deadlines")
}

fn loaded() -> LoadedSystem {
    read_system(&design()).expect("reads")
}

fn config(seconds: f64) -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 150,
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

/// The design defines its own behaviour, and the solver honours it.
///
/// Loading a project-local behaviour and then solving without it is a specific
/// and quiet failure: the design reports a flow nothing rewrote, which looks
/// like an answer rather than like the missing policy it is. This asserts the
/// definition is both loaded and used.
#[test]
fn the_design_defines_and_uses_its_own_behaviour() {
    let system = loaded();
    assert!(
        system.mutators.contains_key("cancellation-propagation"),
        "the design's own behaviour must be loaded"
    );
    // Solving at all proves it reached the evaluator: a behaviour the solver
    // cannot resolve is refused rather than ignored.
    let solved = at(2.0, None);
    assert!(scalar(&solved["browsers"], "success") > 0.9);
}

/// Fanning out is what makes an abandoned request expensive.
///
/// With one operation per request nothing congests at all. The same design
/// asking for six is where the deadline starts firing, because six branches
/// hold six operations open rather than one.
#[test]
fn follow_on_work_is_what_causes_the_congestion() {
    let many = scalar(&at(12.0, None)["index"], "latency");
    let one = scalar(&at(12.0, Some("single-operation"))["index"], "latency");
    assert!(
        many > one * 10.0,
        "expected fan-out to drive the congestion, got {many} against {one}"
    );
}

/// Not propagating the deadline does not change what the user sees.
///
/// This is the trap. Every deadline in the design still fires and the failure
/// rate is unchanged, so the naive implementation looks correct from the only
/// place most teams measure.
#[test]
fn failing_to_propagate_is_invisible_to_the_caller() {
    let propagated = scalar(&at(12.0, None)["browsers"], "success");
    let leaf = scalar(&at(12.0, Some("leaf-timeouts"))["browsers"], "success");
    assert!(
        (propagated - leaf).abs() < 0.02,
        "expected the caller to see no difference, got {propagated} against {leaf}"
    );
}

/// What it changes is how much of the system is occupied doing nothing useful.
///
/// The work carries on being done after the answer has been thrown away, so the
/// service holding it is busy with requests that no longer have anybody waiting
/// for them. That capacity is gone whether or not any dashboard shows it.
#[test]
fn failing_to_propagate_wastes_the_dependency() {
    let propagated = scalar(&at(12.0, None)["search"], "utilisation");
    let leaf = scalar(&at(12.0, Some("leaf-timeouts"))["search"], "utilisation");
    assert!(
        leaf > propagated * 1.5,
        "expected abandoned work to occupy the service, got {propagated} propagated \
         and {leaf} with leaf timeouts"
    );
}

/// The waste persists after the surge that caused it has passed.
#[test]
fn the_waste_outlasts_the_surge() {
    let propagated = scalar(&at(20.0, None)["search"], "utilisation");
    let leaf = scalar(&at(20.0, Some("leaf-timeouts"))["search"], "utilisation");
    assert!(
        leaf > propagated,
        "expected the difference to persist, got {propagated} against {leaf}"
    );
}
