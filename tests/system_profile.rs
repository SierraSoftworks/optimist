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
    profile::{Counter, reset, snapshot, spans},
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
    let spans = spans();

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
    // Phases nest and are summed across threads, so each is shown against the
    // wall clock rather than against the others.
    for (phase, spent) in spans.entries() {
        let name = format!("{phase:?}");
        println!(
            "  {name:<14} {:>12.2?}  ({:>9.1}% of wall clock)",
            spent,
            100.0 * spent.as_secs_f64() / elapsed.as_secs_f64()
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

/// A long transient horizon at the step length the guide asks authors to use.
fn walked(shares: usize) -> EvaluationConfig {
    EvaluationConfig {
        horizon: 300,
        step: 0.05,
        mode: SolveMode::Transient,
        shares,
        ..steady(1_000)
    }
}

/// Dividing the draws changes how many passes each share needs, not only how
/// many run at once, because the damping follows the worst draw anywhere. A
/// share holding only well-behaved draws settles in far fewer passes than the
/// whole ensemble does, so the speed-up is not the share count.
#[test]
fn what_dividing_the_draws_changes() {
    for shares in [1, 2, 4, 8] {
        report(
            &format!("queued-collapse, transient, 300 steps, {shares} share(s)"),
            walked(shares),
            "queued-collapse",
        );
    }
}

/// How much of a pass is the draws, and how much of it would be paid anyway.
///
/// A pass costs the same in name lookups and dictionary copying however many
/// draws it carries, so the part that does not move with the draw count is the
/// part every extra share has to pay again.
#[test]
fn what_a_pass_costs_before_any_draws() {
    for sample_count in [125, 250, 500, 1_000, 2_000] {
        report(
            &format!("queued-collapse, transient, 300 steps, {sample_count} draws"),
            EvaluationConfig {
                sample_count,
                ..walked(1)
            },
            "queued-collapse",
        );
    }
}

/// What the damping ceiling costs in passes.
///
/// A ceiling low enough to keep the worst design from oscillating is paid for by
/// every design that would have settled at a longer stride, and the bill is
/// large: the shipped examples take between four and twenty times the passes at
/// the shipped ceiling than at no ceiling at all.
///
/// It cannot simply be raised. Damping does not only choose the path to a fixed
/// point, it decides which fixed point a design with more than one is able to
/// reach, so read the pass counts here as the price of an answer rather than as
/// a saving that is available. See `system_saturation` for what raising it
/// costs.
#[test]
fn what_the_damping_ceiling_costs() {
    for (damping, tolerance) in [(0.2, 1e-6), (1.0, 1e-6), (1.0, 1e-9), (1.0, 1e-12)] {
        for example in ["queued-collapse", "metastable", "checkout"] {
            report(
                &format!(
                    "{example}, transient, 300 steps, damping {damping}, tolerance {tolerance:e}"
                ),
                EvaluationConfig {
                    damping,
                    tolerance,
                    ..walked(1)
                },
                example,
            );
        }
    }
}
