//! What a solve costs, on the shipped examples.
//!
//! The examples are the only designs whose shape is guaranteed to stay
//! meaningful, so they stand in for real work. Three questions are asked
//! separately because they have different answers: what one settled solve costs,
//! what advancing through time costs on top of that, and how the cost grows with
//! the draw count.
//!
//! Loading and parsing happen outside the timed closure. The measurement is of
//! solving, not of reading YAML.

use std::{collections::BTreeMap, path::PathBuf};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use optimist::system::{
    ComponentType, EvaluationConfig, InterventionId, Mutator, Solve, SolveMode, SystemModel,
    bottlenecks_with_mutators, read_system,
};

/// Examples worth timing, cheapest first.
const EXAMPLES: [&str; 5] = [
    "checkout",
    "saturation",
    "deadlines",
    "queued-collapse",
    "metastable",
];

/// A design loaded and ready to solve, so reading YAML stays out of the measurement.
struct Design {
    model: SystemModel,
    types: BTreeMap<String, ComponentType>,
    mutators: BTreeMap<String, Mutator>,
}

fn design(name: &str) -> Design {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);
    let loaded = read_system(&path).unwrap_or_else(|error| panic!("reads {name}: {error}"));
    Design {
        model: loaded.model,
        types: loaded.component_types,
        mutators: loaded.mutators,
    }
}

fn solve(design: &Design, config: EvaluationConfig) {
    let evaluation = Solve::new(&design.model, &design.types)
        .mutators(&design.mutators)
        .with(config)
        .evaluate()
        .expect("solves");
    std::hint::black_box(evaluation.settled().movement);
}

/// One settled solve of each example, at the default draw count.
fn steady(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("steady");
    group.sample_size(20);
    for name in EXAMPLES {
        let design = design(name);
        let config = EvaluationConfig {
            seed: 0,
            sample_count: 1_000,
            ..EvaluationConfig::default()
        };
        group.bench_function(name, |bencher| {
            bencher.iter(|| solve(&design, config));
        });
    }
    group.finish();
}

/// Advancing through time, which repeats the whole relaxation at every step.
fn transient(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("transient");
    group.sample_size(10);
    for name in ["checkout", "metastable"] {
        let design = design(name);
        for horizon in [10_usize, 60] {
            let config = EvaluationConfig {
                seed: 0,
                sample_count: 1_000,
                horizon,
                mode: SolveMode::Transient,
                ..EvaluationConfig::default()
            };
            group.bench_with_input(BenchmarkId::new(name, horizon), &horizon, |bencher, _| {
                bencher.iter(|| solve(&design, config));
            });
        }
    }
    group.finish();
}

/// How the cost grows with the draw count, which decides whether 10k is affordable.
fn draws(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("draws");
    group.sample_size(10);
    let design = design("checkout");
    for sample_count in [1_000_usize, 4_000, 10_000] {
        let config = EvaluationConfig {
            seed: 0,
            sample_count,
            ..EvaluationConfig::default()
        };
        group.bench_with_input(
            BenchmarkId::from_parameter(sample_count),
            &sample_count,
            |bencher, _| bencher.iter(|| solve(&design, config)),
        );
    }
    group.finish();
}

/// A long horizon at the step length the guide asks authors to use.
///
/// The shipped transient benchmarks step a whole second, which no design being
/// walked through time is faithful at. This is the regime an author actually
/// works in, and it costs a different order of magnitude.
fn walking(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("walking");
    group.sample_size(10);
    for name in ["metastable", "queued-collapse"] {
        let design = design(name);
        for horizon in [60_usize, 300] {
            let config = EvaluationConfig {
                seed: 0,
                sample_count: 1_000,
                horizon,
                step: 0.05,
                mode: SolveMode::Transient,
                ..EvaluationConfig::default()
            };
            group.bench_with_input(BenchmarkId::new(name, horizon), &horizon, |bencher, _| {
                bencher.iter(|| solve(&design, config));
            });
        }
    }
    group.finish();
}

/// What dividing the draws buys, and where it stops buying anything.
///
/// A share repeats every part of a pass that does not depend on the draw count,
/// so this curve flattens and then reverses. Where it turns is the measure of
/// how much of a pass is fixed cost.
fn shares(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("shares");
    group.sample_size(10);
    for name in ["checkout", "queued-collapse"] {
        let design = design(name);
        for shares in [1_usize, 2, 4, 8] {
            let config = EvaluationConfig {
                seed: 0,
                sample_count: 1_000,
                horizon: 300,
                step: 0.05,
                mode: SolveMode::Transient,
                shares,
                ..EvaluationConfig::default()
            };
            group.bench_with_input(BenchmarkId::new(name, shares), &shares, |bencher, _| {
                bencher.iter(|| solve(&design, config));
            });
        }
    }
    group.finish();
}

/// Ranking constraints, which every report does on top of a solve.
fn ranking(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bottlenecks");
    group.sample_size(20);
    for name in ["checkout", "metastable"] {
        let design = design(name);
        let config = EvaluationConfig {
            seed: 0,
            sample_count: 1_000,
            ..EvaluationConfig::default()
        };
        let evaluation = Solve::new(&design.model, &design.types)
            .mutators(&design.mutators)
            .with(config)
            .evaluate()
            .expect("solves");
        let settled = evaluation.settled();
        group.bench_function(name, |bencher| {
            bencher.iter(|| {
                let ranked = bottlenecks_with_mutators(
                    &design.model,
                    &design.types,
                    &design.mutators,
                    settled,
                    config,
                )
                .expect("ranks");
                std::hint::black_box(ranked.len());
            });
        });
    }
    group.finish();
}

/// Weighing every proposal a design carries against the same unchanged baseline.
fn proposals(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("proposals");
    group.sample_size(10);
    for name in ["checkout", "saturation"] {
        let design = design(name);
        let wanted: Vec<InterventionId> = design
            .model
            .interventions
            .iter()
            .map(|intervention| intervention.id.clone())
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let config = EvaluationConfig {
            seed: 0,
            sample_count: 1_000,
            ..EvaluationConfig::default()
        };
        group.bench_with_input(
            BenchmarkId::new(name, wanted.len()),
            &wanted,
            |bencher, wanted| {
                bencher.iter(|| {
                    let weighed = Solve::new(&design.model, &design.types)
                        .mutators(&design.mutators)
                        .with(config)
                        .compare_many(wanted)
                        .expect("compares");
                    std::hint::black_box(weighed.len());
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches, steady, transient, walking, shares, draws, ranking, proposals
);
criterion_main!(benches);
