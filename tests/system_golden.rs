//! What the shipped examples settle on, recorded so a change has to admit it.
//!
//! Most of the work planned for the solver is meant to arrive at the same fixed
//! point by a cheaper route. That is a claim which is easy to make and easy to
//! get wrong: a damping change, a reordered reduction or a different draw
//! partition can move a design onto another branch entirely, and a wall-clock
//! win that quietly rewrote every answer is not a win.
//!
//! Each example's converged channels are therefore summarised and compared
//! against a recorded baseline. Summaries rather than raw draws, because the
//! question is whether the design still says the same thing, and quantiles say
//! that while being robust to the last bit of a floating-point sum. Run with
//! `UPDATE_GOLDEN=1` to re-record after a change that is meant to move them.
//!
//! Every example is solved to record its summary, so this is a
//! `comprehensive_tests` binary.
#![cfg(feature = "comprehensive_tests")]

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{EvaluationConfig, Solve, read_system},
};
use serde::{Deserialize, Serialize};

/// Largest relative disagreement treated as the same answer.
///
/// The solver stops when no draw moves by more than `tolerance`, which defaults
/// to 1e-6, so two runs that both converged are entitled to disagree by about
/// that much without either being wrong.
const AGREEMENT: f64 = 1e-6;

/// Smallest magnitude compared relatively rather than absolutely.
const NEGLIGIBLE: f64 = 1e-12;

const EXAMPLES: [&str; 5] = [
    "checkout",
    "deadlines",
    "metastable",
    "queued-collapse",
    "saturation",
];

/// One channel's converged value, described by where its draws sit.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
struct Summary {
    mean: f64,
    minimum: f64,
    lower: f64,
    median: f64,
    upper: f64,
    maximum: f64,
}

impl Summary {
    /// Describes a value, or returns `None` for one that carries no draws.
    fn of(value: &Value) -> Option<Self> {
        match value {
            Value::Number(number) => Some(Self {
                mean: *number,
                minimum: *number,
                lower: *number,
                median: *number,
                upper: *number,
                maximum: *number,
            }),
            Value::Distribution(distribution) => Some(Self {
                mean: distribution.mean().ok()?,
                minimum: distribution.minimum().ok()?,
                lower: distribution.quantile(0.1).ok()?,
                median: distribution.quantile(0.5).ok()?,
                upper: distribution.quantile(0.9).ok()?,
                maximum: distribution.maximum().ok()?,
            }),
            _ => None,
        }
    }

    /// Names the first statistic that disagrees by more than [`AGREEMENT`].
    fn disagreement(&self, other: &Self) -> Option<String> {
        let pairs = [
            ("mean", self.mean, other.mean),
            ("minimum", self.minimum, other.minimum),
            ("lower", self.lower, other.lower),
            ("median", self.median, other.median),
            ("upper", self.upper, other.upper),
            ("maximum", self.maximum, other.maximum),
        ];
        pairs.into_iter().find_map(|(name, recorded, found)| {
            let scale = recorded.abs().max(found.abs());
            let apart = (recorded - found).abs();
            let agrees = if scale <= NEGLIGIBLE {
                apart <= NEGLIGIBLE
            } else {
                apart / scale <= AGREEMENT
            };
            (!agrees).then(|| format!("{name}: recorded {recorded}, found {found}"))
        })
    }
}

type Recorded = BTreeMap<String, BTreeMap<String, Summary>>;

fn settled(example: &str) -> Recorded {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(example);
    let loaded = read_system(&path).unwrap_or_else(|error| panic!("reads {example}: {error}"));
    let config = EvaluationConfig {
        seed: 0,
        sample_count: 1_000,
        ..EvaluationConfig::default()
    };
    let evaluation = Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(config)
        .evaluate()
        .unwrap_or_else(|error| panic!("solves {example}: {error}"));
    evaluation
        .settled()
        .components
        .iter()
        .map(|(component, state)| {
            let channels = state
                .channels
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), Summary::of(value)?)))
                .collect();
            (component.to_string(), channels)
        })
        .collect()
}

fn baseline(example: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{example}.json"))
}

fn check(example: &str) {
    let found = settled(example);
    let path = baseline(example);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden directory")).expect("creates");
        let recorded = serde_json::to_string_pretty(&found).expect("serialises");
        std::fs::write(&path, format!("{recorded}\n")).expect("writes");
        return;
    }
    let text = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "no baseline for {example} at {}: {error}. Run with UPDATE_GOLDEN=1 to record one.",
            path.display()
        )
    });
    let recorded: Recorded = serde_json::from_str(&text).expect("parses baseline");

    let missing: Vec<_> = recorded
        .keys()
        .filter(|component| !found.contains_key(*component))
        .collect();
    assert!(missing.is_empty(), "{example} no longer solves {missing:?}");

    for (component, channels) in &recorded {
        let solved = &found[component];
        for (channel, summary) in channels {
            let Some(current) = solved.get(channel) else {
                panic!("{example}: {component} no longer has channel '{channel}'");
            };
            if let Some(difference) = summary.disagreement(current) {
                panic!("{example}: {component}.{channel} moved — {difference}");
            }
        }
    }
}

#[test]
fn the_checkout_example_settles_where_it_did() {
    check("checkout");
}

#[test]
fn the_deadlines_example_settles_where_it_did() {
    check("deadlines");
}

#[test]
fn the_metastable_example_settles_where_it_did() {
    check("metastable");
}

#[test]
fn the_queued_collapse_example_settles_where_it_did() {
    check("queued-collapse");
}

#[test]
fn the_saturation_example_settles_where_it_did() {
    check("saturation");
}

/// Every example is covered, so adding one cannot silently go unguarded.
#[test]
fn every_example_has_a_baseline() {
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    let found: Vec<String> = std::fs::read_dir(&examples)
        .expect("reads examples")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .is_dir()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    for example in &found {
        assert!(
            EXAMPLES.contains(&example.as_str()),
            "example '{example}' has no golden baseline test"
        );
    }
}
