//! Where a solve spends its work, counted rather than timed.
//!
//! Timing says a solve is slow. It does not say whether the solver took too many
//! passes or made each pass too expensive, and the two call for opposite work.
//! This reports the counters behind the `profiling` feature so that question can
//! be answered before anything is changed.
//!
//! Run it with:
//!
//! ```text
//! cargo test --release --features profiling --test system_profile -- --nocapture
//! ```

#![cfg(feature = "profiling")]

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    profile::{Counter, reset, snapshot},
    system::{EvaluationConfig, Solve, SolveMode, read_system},
};

fn report(label: &str, config: EvaluationConfig, example: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(example);
    let loaded = read_system(&path).expect("reads");
    reset();
    let started = std::time::Instant::now();
    let evaluation = Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(config)
        .evaluate()
        .expect("solves");
    let elapsed = started.elapsed();
    let counts = snapshot();

    let passes = counts.get(Counter::Passes).max(1);
    println!(
        "\n{label} — {elapsed:.2?}, settled: {}",
        evaluation.settled().converged
    );
    for (counter, total) in counts.entries() {
        let name = format!("{counter:?}");
        println!(
            "  {name:<14} {total:>12}  ({:>10.1} per pass)",
            total as f64 / passes as f64
        );
    }
}

fn steady(sample_count: usize) -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count,
        ..EvaluationConfig::default()
    }
}

#[test]
fn where_a_solve_spends_its_work() {
    report("checkout, steady, 1k draws", steady(1_000), "checkout");
    report("checkout, steady, 10k draws", steady(10_000), "checkout");
    report("metastable, steady, 1k draws", steady(1_000), "metastable");
    report(
        "checkout, transient, 60 steps, 1k draws",
        EvaluationConfig {
            horizon: 60,
            mode: SolveMode::Transient,
            ..steady(1_000)
        },
        "checkout",
    );
    // Past step six this design stops settling, which is the case that used to
    // cost a hundred times what its neighbours did.
    report(
        "saturation, transient, 20 steps, 1k draws",
        EvaluationConfig {
            horizon: 20,
            mode: SolveMode::Transient,
            ..steady(1_000)
        },
        "saturation",
    );
}
