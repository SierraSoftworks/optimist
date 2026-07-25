//! Measures state relation evaluation throughput at projection scale.
//!
//! Node equations are evaluated once per state, per period, per Monte Carlo
//! draw, so their per-call cost decides whether a projection fits the server's
//! execution budget. Run this after touching the interpreter's scoping, module
//! registration, or evaluation loop:
//!
//! ```sh
//! cargo run --release --example relation_throughput
//! ```
//!
//! The workload is 11 states x 12 periods x 1,000 draws, matching the shape of a
//! small real project. Building the standard environment per evaluation rather
//! than once per runtime previously made this nine times slower and pushed it
//! past the two-second analysis budget.

use std::collections::BTreeMap;
use std::time::Instant;

use optimist::domain::{RelationBindings, RelationProgram, RelationSchema, Unit};

fn main() {
    let mut schema =
        RelationSchema::new(Unit::from_exponents([("minute", 1), ("year", -1)]).unwrap());
    schema.parents.insert(
        "outage_frequency".to_owned(),
        Unit::from_exponents([("outage", 1), ("year", -1)]).unwrap(),
    );
    schema.parents.insert(
        "impact_duration".to_owned(),
        Unit::from_exponents([("minute", 1), ("outage", -1)]).unwrap(),
    );

    let compile_start = Instant::now();
    let program = RelationProgram::compile("outage_frequency * impact_duration", &schema).unwrap();
    println!("compile: {:?}", compile_start.elapsed());

    let bindings = RelationBindings {
        baseline: 0.0,
        parents: BTreeMap::from([
            ("outage_frequency".to_owned(), 6.0),
            ("impact_duration".to_owned(), 90.0),
        ]),
        ..RelationBindings::default()
    };

    // 11 nodes x 12 periods x 1000 draws, the shape of a real projection.
    let evaluations = 11 * 12 * 1_000;
    let mut runtime = RelationProgram::runtime(42).unwrap();
    let start = Instant::now();
    let mut total = 0.0;
    for _ in 0..evaluations {
        total += program.evaluate(&mut runtime, &bindings).unwrap();
    }
    let elapsed = start.elapsed();
    println!(
        "{evaluations} evaluations in {elapsed:?} ({:.1} us each), checksum {total}",
        elapsed.as_secs_f64() * 1e6 / f64::from(evaluations)
    );
}
