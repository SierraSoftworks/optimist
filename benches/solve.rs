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
    ComponentType, EvaluationConfig, Mutator, SolveMode, SystemModel, evaluate_with_mutators,
    read_system,
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
    let evaluation = evaluate_with_mutators(
        &design.model,
        &design.types,
        &design.mutators,
        &BTreeMap::new(),
        config,
    )
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

criterion_group!(benches, steady, transient, draws);
criterion_main!(benches);
