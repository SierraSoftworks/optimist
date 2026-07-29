//! What one Squiggle program costs, apart from the solve that runs it.
//!
//! A solve is tens of thousands of evaluations of a handful of very short
//! expressions, so its cost is the interpreter's per-program overhead multiplied
//! by a number the solver chooses. `benches/solve.rs` measures the product;
//! this measures the multiplicand, which is the only half an interpreter change
//! can move.
//!
//! The programs are the shipped catalogue's, verbatim, and the scope they run
//! against has the shape `evaluate_component` builds: globals, properties, `t`,
//! `dt`, `steady`, and the nested `in`/`out`/`prev` dictionaries.
//!
//! Two draw counts are timed for a reason. At one draw the distribution algebra
//! costs nothing measurable, so what is left is interpretation: scope
//! allocation, name resolution, value copying, dispatch. At a thousand draws the
//! arithmetic dominates. A change that helps only the first will look free in
//! the second, and a change that helps only the second says nothing about the
//! floor.

use std::collections::BTreeMap;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use optimist::squiggle::{
    Distribution, Runtime, RuntimeConfig, Value,
    ast::Program,
    parse,
};

/// Catalogue expressions, chosen to separate the costs they exercise.
const PROGRAMS: [(&str, &str); 8] = [
    ("field", "in.requests.rate"),
    ("scalar", "service_time + dependency_wait"),
    ("guarded", "max([arriving - salvaged, 0])"),
    (
        "queue",
        "min([arrivals + prev.backlog / dt, service_rate])",
    ),
    ("namespaced", "Little.occupancy(offered, residence)"),
    ("stationary", "Queue.utilisation(offered, capacity)"),
    (
        "branching",
        "if steady\n  then Queue.boundedLength(load, capacity)\n  else min([max([prev.backlog + (arrivals * accepted_ratio - departures) * dt, 0]), capacity])",
    ),
    (
        "ratio",
        "min([(max([capacity - prev.backlog, 0]) / dt + departures) / max([arrivals, 0.000001]), 1])",
    ),
];

/// A sample set standing in for a quantity an upstream component published.
///
/// Solves pass sampled quantities between components, so a benchmark binding
/// symbolic families instead would measure inverse-transform sampling that the
/// hot path has already done once and cached.
fn drawn(sample_count: usize, centre: f64) -> Value {
    let samples = (0..sample_count)
        .map(|index| centre * (1.0 + 0.2 * ((index as f64) / (sample_count as f64) - 0.5)))
        .collect();
    Value::Distribution(Distribution::from_samples(samples).expect("finite draws"))
}

fn dictionary<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Dictionary(
        entries
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect::<BTreeMap<_, _>>(),
    )
}

/// Binds the scope `evaluate_component` builds, once per runtime.
fn scoped(sample_count: usize) -> Runtime {
    let config = RuntimeConfig {
        seed: 0,
        sample_count,
        ..RuntimeConfig::default()
    };
    let mut runtime = Runtime::with_config(config).expect("valid configuration");
    let bindings: Vec<(&str, Value)> = vec![
        ("t", Value::Number(0.0)),
        ("dt", Value::Number(1.0)),
        ("steady", Value::Boolean(true)),
        ("service_rate", Value::Number(800.0)),
        ("capacity", Value::Number(1_200.0)),
        ("service_time", drawn(sample_count, 0.04)),
        ("dependency_wait", drawn(sample_count, 0.01)),
        ("arriving", drawn(sample_count, 500.0)),
        ("salvaged", drawn(sample_count, 12.0)),
        ("arrivals", drawn(sample_count, 500.0)),
        ("departures", drawn(sample_count, 480.0)),
        ("accepted_ratio", drawn(sample_count, 0.98)),
        ("offered", drawn(sample_count, 500.0)),
        ("residence", drawn(sample_count, 0.05)),
        ("load", drawn(sample_count, 0.6)),
        (
            "in",
            dictionary([(
                "requests",
                dictionary([
                    ("rate", drawn(sample_count, 500.0)),
                    ("cancellation", drawn(sample_count, 0.02)),
                    ("cancellation_effectiveness", Value::Number(0.5)),
                ]),
            )]),
        ),
        (
            "out",
            dictionary([(
                "dependencies",
                dictionary([
                    ("latency", drawn(sample_count, 0.01)),
                    ("success", drawn(sample_count, 0.999)),
                ]),
            )]),
        ),
        (
            "prev",
            dictionary([
                ("backlog", drawn(sample_count, 20.0)),
                ("departures", drawn(sample_count, 480.0)),
                ("load", drawn(sample_count, 0.6)),
            ]),
        ),
    ];
    for (name, value) in bindings {
        runtime.bench_bind(name, value);
    }
    runtime
}

fn parsed() -> Vec<(&'static str, Program)> {
    PROGRAMS
        .iter()
        .map(|(name, source)| {
            (
                *name,
                parse(source).unwrap_or_else(|error| panic!("parses {name}: {error:?}")),
            )
        })
        .collect()
}

/// Interpretation with the arithmetic turned down to nothing.
fn floor(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("program/1");
    let programs = parsed();
    let mut runtime = scoped(1);
    for (name, program) in &programs {
        group.bench_function(*name, |bencher| {
            bencher.iter(|| {
                std::hint::black_box(
                    runtime
                        .bench_evaluate(program)
                        .unwrap_or_else(|error| panic!("evaluates {name}: {}", error.message)),
                );
            });
        });
    }
    group.finish();
}

/// The same programs at the draw count a solve defaults to on the bench.
fn drawing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("program/1000");
    let programs = parsed();
    let mut runtime = scoped(1_000);
    for (name, program) in &programs {
        group.bench_function(*name, |bencher| {
            bencher.iter(|| {
                std::hint::black_box(
                    runtime
                        .bench_evaluate(program)
                        .unwrap_or_else(|error| panic!("evaluates {name}: {}", error.message)),
                );
            });
        });
    }
    group.finish();
}

/// A whole component's worth of channels, which is the unit a pass repeats.
fn component(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("component");
    let programs = parsed();
    for sample_count in [1_usize, 1_000] {
        let mut runtime = scoped(sample_count);
        group.bench_with_input(
            BenchmarkId::from_parameter(sample_count),
            &sample_count,
            |bencher, _| {
                bencher.iter(|| {
                    for (name, program) in &programs {
                        std::hint::black_box(
                            runtime.bench_evaluate(program).unwrap_or_else(|error| {
                                panic!("evaluates {name}: {}", error.message)
                            }),
                        );
                    }
                });
            },
        );
    }
    group.finish();
}

/// Parsing, which happens once per compiled program rather than per evaluation.
fn parsing(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parse");
    for (name, source) in PROGRAMS {
        group.bench_function(name, |bencher| {
            bencher.iter(|| std::hint::black_box(parse(source).expect("parses")));
        });
    }
    group.finish();
}

criterion_group!(benches, floor, drawing, component, parsing);
criterion_main!(benches);
