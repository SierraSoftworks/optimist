//! The worked example under `examples/metastable`, checked end to end.
//!
//! The example exists to demonstrate that a system can have two steady states at
//! one level of demand, and that the thresholds bounding them are not where
//! intuition puts them. These tests guard those conclusions rather than the exact
//! numbers, because the example is documentation and a change that makes it stop
//! teaching what it claims is a defect in the example or the engine.
//!
//! Where a number is asserted it is one the physics fixes analytically, so the
//! assertion is checking the solver against theory rather than against itself.

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{
        ComponentState, EvaluationConfig, InterventionId, LoadedSystem, bottlenecks, compare,
        evaluate, evaluate_intervention, read_system,
    },
};

/// Connections the dependency holds open, `C`.
const CONNECTIONS: f64 = 220.0;
/// Sequential dependency calls per request, `d`.
const DEPTH: f64 = 8.0;
/// Uncongested service time of one call, `s`.
const SERVICE: f64 = 0.05;
/// Budget for a complete request, `D`.
const DEADLINE: f64 = 2.0;

fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/metastable")
}

fn loaded() -> LoadedSystem {
    read_system(&design()).expect("reads")
}

/// Long enough for the surge to have started, ended, and been lived with.
fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 1_000,
        horizon: 25,
        step: 1.0,
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

fn settled(intervention: Option<&str>) -> BTreeMap<String, ComponentState> {
    let system = loaded();
    let evaluation = match intervention {
        None => evaluate(&system.model, &system.component_types, config()),
        Some(id) => evaluate_intervention(
            &system.model,
            &system.component_types,
            &InterventionId::new(id),
            config(),
        ),
    }
    .expect("solves");
    evaluation
        .settled()
        .components
        .iter()
        .map(|(id, state)| (id.to_string(), state.clone()))
        .collect()
}

/// The example loads, including the component types it defines for itself.
///
/// Both types are project-local. That is the claim being checked: the physics
/// this example needs was added as two manifests in its own directory, with no
/// change to the solver.
#[test]
fn the_example_defines_its_own_component_types() {
    let system = loaded();
    assert_eq!(system.name, "Metastable saturation");
    assert!(system.component_types.contains_key("request-handler"));
    assert!(system.component_types.contains_key("connection-pool"));

    let handler = &system.component_types["request-handler"];
    assert!(
        handler.outputs.contains_key("occupancy"),
        "the caller must publish what it holds, or the loop cannot close"
    );
    let pool = &system.component_types["connection-pool"];
    assert!(
        pool.outputs.contains_key("added_latency"),
        "the pool must return latency, or the loop cannot close"
    );
}

/// Undisturbed, the design sits comfortably inside its limits.
///
/// The analytic healthy root of `u(1-u) = load * depth * service / connections`
/// is checked directly, because every later claim rests on the solver having
/// found the right branch rather than merely a settled one.
#[test]
fn at_rest_the_design_settles_on_the_healthy_branch() {
    let system = loaded();
    let evaluation = evaluate_intervention(
        &system.model,
        &system.component_types,
        &InterventionId::new("no-surge"),
        config(),
    )
    .expect("solves");
    assert!(evaluation.converged(), "the resting design must settle");

    let state = evaluation.settled();
    let pool = state
        .components
        .iter()
        .find(|(id, _)| id.as_str() == "inventory")
        .map(|(_, state)| state)
        .expect("the pool");

    let offered = 100.0 * DEPTH * SERVICE / CONNECTIONS;
    let expected = (1.0 - (1.0 - 4.0 * offered).sqrt()) / 2.0;
    let utilisation = scalar(pool, "utilisation");
    assert!(
        (utilisation - expected).abs() < 0.02,
        "expected the lower root near {expected:.3}, got {utilisation:.3}"
    );
    assert!(
        utilisation < 0.5,
        "the healthy branch never exceeds half occupancy, got {utilisation:.3}"
    );
}

/// The design does not return to health when the surge that broke it ends.
///
/// This is the whole point of the example. The baseline and the `no-surge`
/// counterfactual are offered exactly the same demand at the moment they are
/// read; they differ only in what happened ten seconds earlier, and they settle
/// on different states.
#[test]
fn the_design_stays_collapsed_after_the_surge_that_caused_it_has_passed() {
    let after = settled(None);
    let never = settled(Some("no-surge"));

    let arrivals = scalar(&after["checkout"], "arrivals");
    let quiet_arrivals = scalar(&never["checkout"], "arrivals");
    assert!(
        (arrivals - quiet_arrivals).abs() < 1e-6,
        "both must be offered the same demand to make the point, {arrivals} against {quiet_arrivals}"
    );

    let collapsed = scalar(&after["inventory"], "utilisation");
    let healthy = scalar(&never["inventory"], "utilisation");
    assert!(
        collapsed > 0.85,
        "the aftermath must still be congested, got {collapsed:.3}"
    );
    assert!(
        healthy < 0.35,
        "the counterfactual must be comfortable, got {healthy:.3}"
    );

    assert!(
        scalar(&after["checkout"], "success_rate") < 0.2,
        "requests must be failing in the aftermath"
    );
    assert!(
        scalar(&never["checkout"], "success_rate") > 0.98,
        "requests must be served in the counterfactual"
    );
}

/// Collapsed, every request holds a connection for exactly its whole budget.
///
/// The collapsed branch is pinned by the deadline rather than by the connection
/// limit, which is why occupancy settles at `load * deadline` and the pool is
/// left saturated but not exhausted. A design read only through its connection
/// limit would conclude there was headroom.
#[test]
fn the_collapsed_state_is_held_in_place_by_the_deadline_not_the_pool() {
    let after = settled(None);
    let holding = scalar(&after["checkout"], "holding");
    assert!(
        (holding - DEADLINE).abs() < 1e-6,
        "collapsed requests squat for the full deadline, got {holding}"
    );

    let held = scalar(&after["checkout"], "connections");
    let expected = 100.0 * DEADLINE;
    assert!(
        (held - expected).abs() < 1.0,
        "occupancy is demand times deadline, expected {expected}, got {held}"
    );
    assert!(
        held < CONNECTIONS,
        "the pool is not actually exhausted, which is what makes this hard to spot"
    );

    let system = loaded();
    let evaluation = evaluate(&system.model, &system.component_types, config()).expect("solves");
    let ranked = bottlenecks(
        &system.model,
        &system.component_types,
        evaluation.settled(),
        config(),
    )
    .expect("ranks");
    let deadline = ranked
        .iter()
        .find(|entry| entry.constraint == "deadline")
        .expect("a deadline constraint");
    assert!(
        deadline.binds(),
        "the deadline is the constraint actually being violated"
    );
    assert_eq!(
        ranked[0].constraint, "deadline",
        "and it should rank above the pool it is mistaken for"
    );
}

/// Shedding below the release load is the lever that ends the collapse.
///
/// Nothing about the dependency changes; only the demand reaching it does. The
/// released state is reached from a collapsed one, which is what distinguishes
/// this from the interventions that merely make the collapse less painful.
#[test]
fn shedding_under_the_release_load_recovers_and_the_others_do_not() {
    let release = (CONNECTIONS / DEADLINE) * (1.0 - DEPTH * SERVICE / DEADLINE);

    let shed = settled(Some("shed"));
    let admitted = scalar(&shed["checkout"], "arrivals");
    assert!(
        admitted < release,
        "shedding must land under the release load of {release:.0}, got {admitted:.1}"
    );
    assert!(
        scalar(&shed["inventory"], "utilisation") < 0.35,
        "and the pool must actually come back"
    );
    assert!(
        scalar(&shed["checkout"], "success_rate") > 0.98,
        "and requests must be served again"
    );

    let retries = settled(Some("fewer-retries"));
    assert!(
        scalar(&retries["inventory"], "utilisation") > 0.85,
        "cutting retries does not release a collapse the deadline is holding open"
    );
}

/// Lengthening the deadline deepens the trap rather than easing it.
///
/// The release load falls as the deadline rises, because a doomed request holds
/// its connection for longer. This is the example's most counter-intuitive
/// claim and the one most worth guarding.
#[test]
fn giving_requests_longer_makes_the_collapse_worse() {
    let longer = settled(Some("longer-deadline"));
    let baseline = settled(None);

    let before = scalar(&baseline["inventory"], "utilisation");
    let after = scalar(&longer["inventory"], "utilisation");
    assert!(
        after > before,
        "a longer deadline must load the pool further, {before:.3} to {after:.3}"
    );
    assert!(
        after > 1.0,
        "demand for connections must exceed what the pool offers, got {after:.3}"
    );
    assert!(
        scalar(&longer["checkout"], "success_rate")
            <= scalar(&baseline["checkout"], "success_rate"),
        "and no more requests may be served for the trouble"
    );

    let comparison = compare(
        &loaded().model,
        &loaded().component_types,
        &InterventionId::new("longer-deadline"),
        config(),
    )
    .expect("compares");
    assert!(
        comparison.relieved().is_empty(),
        "nothing is relieved by this change, {:#?}",
        comparison.movements
    );
}
