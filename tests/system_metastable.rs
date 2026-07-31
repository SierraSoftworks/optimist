//! The worked example under `examples/metastable`, checked end to end.
//!
//! The example exists to show that a retry policy answering a saturated
//! dependency multiplies the load on it, that the multiplication is emergent
//! rather than assumed, and that a buffer added to absorb bursts makes the
//! episode worse rather than better. These tests guard those conclusions rather
//! than the exact numbers, because the example is documentation and a change
//! that makes it stop teaching what it claims is a defect in the example or the
//! engine.
//!
//! What the example does *not* currently show is hysteresis: it recovers once
//! the surge passes. That is a real property of the model as it stands and is
//! asserted below, so that anyone who later sharpens the saturation law finds a
//! failing test telling them the example's claims have changed rather than
//! discovering it by reading.
//!
//! Each claim costs a transient solve of the whole design, so this is a
//! `comprehensive_tests` binary.
#![cfg(feature = "comprehensive_tests")]

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{
        ComponentState, EvaluationConfig, InterventionId, LoadedSystem, SolveMode, bottlenecks,
        evaluate, evaluate_intervention, read_system,
    },
};

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
        sample_count: 200,
        horizon: 25,
        step: 1.0,
        ..EvaluationConfig::default()
    }
}

/// The same run stopped partway, for reading a moment rather than the end.
fn config_to(horizon: usize) -> EvaluationConfig {
    EvaluationConfig {
        horizon,
        ..config()
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

fn settled(
    intervention: Option<&str>,
    config: EvaluationConfig,
) -> BTreeMap<String, ComponentState> {
    let system = loaded();
    let evaluation = match intervention {
        None => evaluate(&system.model, &system.component_types, config),
        Some(id) => evaluate_intervention(
            &system.model,
            &system.component_types,
            &InterventionId::new(id),
            config,
        ),
    }
    .expect("solves");
    // The step being read has to have settled. Steps in the middle of the surge
    // arriving or leaving need not: demand moves between two very different
    // steady states there, and an iterate chasing a moving target within a
    // single step is not evidence about the state it eventually reaches.
    assert!(
        evaluation.settled().converged,
        "the moment being read must settle, moved {}",
        evaluation.settled().movement
    );
    evaluation
        .settled()
        .components
        .iter()
        .map(|(id, state)| (id.to_string(), state.clone()))
        .collect()
}

/// The example is built from the shipped catalogue and nothing else.
///
/// This is the claim worth guarding. An earlier version of this example needed
/// two component types of its own to express a caller that holds a connection
/// for the whole of a downstream call; responses travelling back along a
/// relationship made both unnecessary. If a bespoke type reappears here, the
/// catalogue has lost something it used to be able to say.
#[test]
fn the_example_uses_only_shipped_component_types() {
    let system = loaded();
    assert_eq!(system.name, "Metastable saturation");
    for component in &system.model.components {
        let id = component.component_type.as_str();
        assert!(
            ["client", "compute", "datastore"].contains(&id),
            "'{}' adopts '{id}', which is not a shipped type",
            component.id
        );
    }
}

/// Undisturbed, the design serves its resting load comfortably.
#[test]
fn at_rest_the_design_is_healthy() {
    let solved = settled(Some("no-surge"), config());
    let checkout = &solved["checkout"];
    assert!(
        scalar(checkout, "utilisation") < 0.8,
        "resting utilisation should leave headroom, got {}",
        scalar(checkout, "utilisation")
    );
    let shoppers = &solved["shoppers"];
    assert!(
        scalar(shoppers, "success") > 0.85,
        "resting success should be high, got {}",
        scalar(shoppers, "success")
    );
}

/// The surge drives the service far past its capacity, and retries do most of it.
///
/// The amplification is the point. Demand rises fivefold; what reaches the
/// service rises considerably further, because every attempt that fails is
/// reissued and failures are what saturation produces.
#[test]
fn retries_amplify_the_surge_beyond_the_demand_that_caused_it() {
    let during = settled(None, config_to(12));
    let offered = scalar(&during["shoppers"], "rate");
    let arriving = scalar(&during["checkout"], "arriving");
    assert!(
        arriving > offered * 2.0,
        "retrying should multiply the surge, {offered} offered but {arriving} arriving"
    );
    assert!(
        scalar(&during["checkout"], "utilisation") > 1.0,
        "the surge must saturate the service for the point to land"
    );
}

/// Latency and success are reported where a user would have experienced them.
///
/// Responses travel back to the caller, so the client's own channels already
/// account for every hop, retry and timeout behind them. This is what makes the
/// design's objectives expressible as constraints rather than as arithmetic
/// somebody does by hand afterwards.
#[test]
fn the_caller_observes_the_whole_system() {
    let during = settled(None, config_to(12));
    let shoppers = &during["shoppers"];
    assert!(
        scalar(shoppers, "success") < 0.5,
        "a saturated system must show up as failure at the caller, got {}",
        scalar(shoppers, "success")
    );

    let system = loaded();
    let evaluation =
        evaluate(&system.model, &system.component_types, config_to(12)).expect("solves");
    let ranked = bottlenecks(
        &system.model,
        &system.component_types,
        evaluation.settled(),
        config_to(12),
    )
    .expect("ranks");
    let worst = &ranked[0];
    assert!(worst.binds(), "something must be binding during the surge");
    let objective = ranked
        .iter()
        .find(|entry| entry.constraint == "success_objective")
        .expect("the caller's objective is ranked");
    assert!(
        objective.binds(),
        "and the objective the design exists to meet must be among them"
    );
}

/// Both solvers agree about where this design ends up.
///
/// The two modes answer different questions — one solves for where the design
/// comes to rest, the other walks it there a step at a time — and they must not
/// disagree about the destination. Advancing through time may take a different
/// route and reveal how long the journey is, but a design that settles collapsed
/// under one mode has to settle collapsed under the other, or one of them is
/// lying.
///
/// The collapse here arrives immediately either way. What holds it is Little's
/// Law relating the store's held connections to its own delay, and that is an
/// identity rather than a queue: there is nothing to fill, so there is nothing
/// to fill gradually. The wire backlogs do integrate, which is what the mode is
/// for, but they are not what traps this design.
#[test]
fn both_modes_agree_the_design_stays_collapsed() {
    let system = loaded();
    let walked = EvaluationConfig {
        mode: SolveMode::Transient,
        step: 0.5,
        sample_count: 40,
        horizon: 60,
        ..config()
    };
    let balanced = EvaluationConfig {
        mode: SolveMode::Steady,
        ..walked
    };

    let solve = |config| {
        evaluate(&system.model, &system.component_types, config)
            .expect("solves")
            .settled()
            .components
            .iter()
            .map(|(id, state)| (id.to_string(), state.clone()))
            .collect::<BTreeMap<_, _>>()
    };

    let by_time = solve(walked);
    let by_balance = solve(balanced);
    for (component, channel) in [("inventory", "concurrency"), ("shoppers", "success")] {
        let walked = scalar(&by_time[component], channel);
        let balanced = scalar(&by_balance[component], channel);
        assert!(
            (walked - balanced).abs() < balanced.abs().max(1.0) * 0.05,
            "the modes disagree about {component}.{channel}: {walked} against {balanced}"
        );
    }
    assert!(
        scalar(&by_time["inventory"], "concurrency") > 1.0,
        "and both should find the collapse"
    );
}

/// The surge leaves the design collapsed, and it stays that way.
///
/// This is the whole point. Demand returns to a level the design served
/// perfectly well a moment earlier, and the design does not return with it: the
/// store is still holding every connection it can, the workers waiting on those
/// connections are still held, and the callers timing out on those workers are
/// still retrying. Each of those facts is caused by the other two.
///
/// Nothing about the configuration changed. Reading the design's properties
/// after the surge would tell you it was fine.
#[test]
fn the_surge_leaves_a_collapse_that_outlasts_it() {
    let after = settled(None, config());
    let store = &after["inventory"];
    assert!(
        scalar(store, "concurrency") > 1.0,
        "the store should still be holding every connection it has, got {}",
        scalar(store, "concurrency")
    );
    let shoppers = &after["shoppers"];
    assert!(
        scalar(shoppers, "success") < 0.2,
        "and callers should still be failing, got {}",
        scalar(shoppers, "success")
    );
    // Demand is back to resting, so the collapse is not being sustained by load.
    assert!((scalar(shoppers, "rate") - 600.0).abs() < 1.0);
}

/// The same design never surged is perfectly healthy at the same demand.
///
/// Run beside the test above this is the entire finding: one design, one level
/// of demand, two lasting outcomes. Which one a system is in cannot be read off
/// its configuration, because the configuration is identical.
#[test]
fn the_same_demand_is_survivable_if_it_was_never_exceeded() {
    let never = settled(Some("no-surge"), config());
    assert!(
        scalar(&never["inventory"], "concurrency") < 0.5,
        "without the surge the store is barely busy, got {}",
        scalar(&never["inventory"], "concurrency")
    );
    assert!(
        scalar(&never["shoppers"], "success") > 0.95,
        "and callers are served, got {}",
        scalar(&never["shoppers"], "success")
    );
}

/// A deeper buffer does not save a design from this.
///
/// Queueing is added to absorb bursts, and it does absorb them; what it cannot
/// do is create the capacity the burst was asking for. Widening the wire tenfold
/// leaves the design in exactly the same collapse, having spent the extra depth
/// on holding requests that were going to fail anyway.
#[test]
fn a_deeper_queue_does_not_prevent_the_collapse() {
    let deep = settled(Some("deep-queue"), config());
    assert!(
        scalar(&deep["inventory"], "concurrency") > 1.0,
        "a deeper wire still ends up collapsed, got {}",
        scalar(&deep["inventory"], "concurrency")
    );
    assert!(
        scalar(&deep["shoppers"], "success") < 0.2,
        "and its callers are no better served"
    );
}

/// Shedding demand rescues the service but not the callers it turns away.
///
/// The uncomfortable half of this is the half worth having. Refusing work does
/// protect the service — utilisation falls, latency returns to normal, and what
/// is admitted is served properly — but a caller that was refused has not been
/// served either, and during a surge this large most of them are. Shedding
/// chooses who to disappoint; it does not conjure capacity, and a model that
/// reported it as an improvement to everyone would be lying about the trade.
#[test]
fn shedding_relieves_the_service_without_rescuing_the_callers() {
    let plain = settled(None, config_to(12));
    let shed = settled(Some("shed"), config_to(12));
    assert!(
        scalar(&shed["checkout"], "utilisation") < scalar(&plain["checkout"], "utilisation"),
        "shedding must relieve the service"
    );
    assert!(
        scalar(&shed["shoppers"], "latency") < scalar(&plain["shoppers"], "latency"),
        "and the requests it does admit must be answered promptly"
    );
    assert!(
        scalar(&shed["shoppers"], "success") < 0.5,
        "but demand this far past capacity cannot all be served, however it is \
         refused, got {}",
        scalar(&shed["shoppers"], "success")
    );
}
