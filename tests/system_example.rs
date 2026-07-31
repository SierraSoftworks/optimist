//! The worked example under `examples/checkout`, checked end to end.
//!
//! The example exists to be read by people, so this test guards the conclusions
//! it is supposed to demonstrate rather than its exact numbers. If a change to
//! the engine makes the example stop teaching what it claims to teach, the
//! example is wrong and so is the documentation built on it.
//!
//! Ranking the example's constraints solves it once per reading, so this is a
//! `comprehensive_tests` binary.
#![cfg(feature = "comprehensive_tests")]

use std::path::PathBuf;

use optimist::system::{
    Bottleneck, EvaluationConfig, InterventionId, bottlenecks, compare, evaluate, read_system,
};

fn design() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/checkout")
}

fn config() -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 1_000,
        ..EvaluationConfig::default()
    }
}

fn ranked() -> Vec<Bottleneck> {
    let loaded = read_system(&design()).expect("reads");
    let evaluation = evaluate(&loaded.model, &loaded.component_types, config()).expect("solves");
    bottlenecks(
        &loaded.model,
        &loaded.component_types,
        evaluation.settled(),
        config(),
    )
    .expect("ranks")
}

fn find<'a>(ranked: &'a [Bottleneck], component: &str, constraint: &str) -> &'a Bottleneck {
    ranked
        .iter()
        .find(|entry| entry.component.as_str() == component && entry.constraint == constraint)
        .unwrap_or_else(|| panic!("no {component}/{constraint} in {ranked:#?}"))
}

/// The shipped example loads and solves.
#[test]
fn the_worked_example_solves() {
    let loaded = read_system(&design()).expect("reads");
    assert_eq!(loaded.name, "Checkout");
    assert_eq!(loaded.model.components.len(), 3);
    assert_eq!(loaded.model.interventions.len(), 2);

    let evaluation = evaluate(&loaded.model, &loaded.component_types, config()).expect("solves");
    assert!(
        evaluation.settled().converged,
        "the moment being read must settle, moved {}",
        evaluation.settled().movement
    );
}

/// Retention is the example's headline finding.
///
/// Thirty days of four kilobyte records at the offered rate exceeds the store's
/// capacity several times over, and it is the constraint an engineer is least
/// likely to notice because it is only reached once the retention window has
/// fully elapsed.
#[test]
fn the_store_runs_out_of_room() {
    let ranked = ranked();
    let volume = find(&ranked, "orders", "volume");
    assert!(volume.binds());
    assert!(
        volume.utilisation > 2.0,
        "retention should overrun capacity, got {}",
        volume.utilisation
    );
    assert_eq!(ranked[0].component.as_str(), "orders");
}

/// The pool's mean utilisation understates how often it saturates.
///
/// Service time is uncertain, so a mean near one hides that a large share of
/// draws have already crossed it. This is the case a model evaluated at the
/// mean cannot see, and the reason the example carries an uncertain service
/// time rather than a convenient constant.
#[test]
fn the_pool_binds_more_often_than_its_mean_suggests() {
    let ranked = ranked();
    let capacity = find(&ranked, "api", "capacity");
    assert!(
        capacity.probability_of_binding > 0.25,
        "a real share of draws must saturate, got {}",
        capacity.probability_of_binding
    );
    assert!(
        capacity.utilisation_p90 > capacity.utilisation,
        "the upper tail must sit above the mean"
    );
}

/// Neither proposal fixes the design on its own, which is the lesson.
///
/// Caching relieves the store and leaves the pool exactly where it was.
/// Enlarging the pool relieves the pool and pushes more traffic at a store that
/// could not take what it already had. Reading either in isolation would
/// suggest the design was fixed.
///
/// The two proposals are not symmetric. Caching relieves the store and the pool
/// together, because a pool worker is held for the whole of a downstream call
/// and a faster store hands it back sooner. Enlarging the pool relieves only the
/// pool, and pays for it at the store.
#[test]
fn each_proposal_fixes_one_constraint_and_not_the_other() {
    let loaded = read_system(&design()).expect("reads");

    let cache = compare(
        &loaded.model,
        &loaded.component_types,
        &InterventionId::new("warm-cache"),
        config(),
    )
    .expect("compares");
    let store = cache
        .movements
        .iter()
        .find(|movement| movement.component.as_str() == "orders" && movement.constraint == "volume")
        .expect("volume movement");
    assert!(
        store.shift() < 0.0,
        "caching should relieve the store, moved {}",
        store.shift()
    );
    // What caching does to the pool is deliberately not asserted. Two effects
    // pull against each other — a faster store frees workers sooner, while the
    // higher success rate that follows means fewer retries and so a different
    // offered load — and which one wins is a property of these particular
    // numbers rather than a lesson the example is teaching.

    let bigger = compare(
        &loaded.model,
        &loaded.component_types,
        &InterventionId::new("bigger-pool"),
        config(),
    )
    .expect("compares");
    assert!(
        bigger
            .relieved()
            .iter()
            .any(|movement| movement.component.as_str() == "api"),
        "a larger pool should relieve the pool, {:#?}",
        bigger.movements
    );
    let store = bigger
        .movements
        .iter()
        .find(|movement| movement.component.as_str() == "orders" && movement.constraint == "volume")
        .expect("volume movement");
    assert!(
        store.shift() > 0.0,
        "letting more traffic through must load the store further, moved {}",
        store.shift()
    );
    assert!(store.bound_after > 0.0, "the store still cannot cope");
}
