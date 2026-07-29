//! Solving a model in shares must give the answer solving it whole would.
//!
//! Every draw index carries its own deterministic system, so dividing the draws
//! across workers is exact rather than approximate. That claim is worth checking
//! against the shipped designs rather than asserting: a share that sampled its
//! own draws, or that read the wrong window of somebody else's, would still
//! produce plausible-looking numbers.
//!
//! Agreement is to the convergence criterion rather than to the last bit. A
//! relaxation stops when its worst draw stops moving, and a share's worst draw is
//! not the model's, so the whole and its shares leave off at slightly different
//! points along the same path. What remains between them scales with the
//! tolerance they were given: solved to `1e-6` the shares agree to about five
//! significant figures, and to `1e-12` they agree to eleven. They are converging
//! on one fixed point, and how near they were asked to get decides how near they
//! land to each other.

use std::{collections::BTreeMap, path::PathBuf};

use optimist::{
    squiggle::Value,
    system::{ComponentState, EvaluationConfig, Solve, SolveMode, Step, read_system},
};

/// Designs with a single resting state, where dividing must reproduce it.
const SETTLING: [&str; 3] = ["checkout", "deadlines", "saturation"];

/// Designs that admit more than one resting state.
///
/// Which one a solve reports depends on the path it takes to get there, and a
/// share damps against its own worst draw rather than the model's, so it can take
/// a different path and arrive on the other branch. That is a property of the
/// design rather than of the division, and it is the same reason the solver's
/// damping cannot be retuned freely.
const BISTABLE: [&str; 2] = ["metastable", "queued-collapse"];

fn solved(example: &str, config: EvaluationConfig) -> Step {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(example);
    let loaded = read_system(&path).unwrap_or_else(|error| panic!("reads {example}: {error}"));
    let evaluation = Solve::new(&loaded.model, &loaded.component_types)
        .mutators(&loaded.mutators)
        .with(config)
        .evaluate()
        .unwrap_or_else(|error| panic!("solves {example}: {error}"));
    evaluation.settled().clone()
}

/// Reads a quantity as the draws it stands for, so a collapsed share and the
/// draws it collapsed from compare equal.
fn spread(value: &Value, width: usize) -> Vec<f64> {
    match value {
        Value::Number(number) => vec![*number; width],
        Value::Distribution(distribution) => distribution
            .samples()
            .map_or_else(|| vec![f64::NAN; width], <[f64]>::to_vec),
        _ => Vec::new(),
    }
}

fn channels(state: &ComponentState, width: usize) -> BTreeMap<String, Vec<f64>> {
    state
        .channels
        .iter()
        .map(|(name, value)| (name.clone(), spread(value, width)))
        .collect()
}

fn agree(example: &str, config: EvaluationConfig, shares: usize) {
    let whole = solved(example, config);
    let divided = solved(example, EvaluationConfig { shares, ..config });

    // A design with no steady state has no fixed point for the shares to agree
    // on: each stops where its own worst draw stopped improving, which is a
    // different place. That it is still reported as unsettled either way is the
    // part worth checking.
    if !whole.converged {
        assert!(
            !divided.converged,
            "{example}: divided into {shares} claimed to settle where the whole did not"
        );
        return;
    }

    assert_eq!(
        whole.components.keys().collect::<Vec<_>>(),
        divided.components.keys().collect::<Vec<_>>(),
        "{example}: divided into {shares} solved different components"
    );
    for (id, expected) in &whole.components {
        let found = &divided.components[id];
        let expected = channels(expected, config.sample_count);
        let found = channels(found, config.sample_count);
        for (name, expected) in &expected {
            let found = found.get(name).unwrap_or_else(|| {
                panic!("{example}: {id}.{name} missing when divided into {shares}")
            });
            assert_eq!(
                expected.len(),
                found.len(),
                "{example}: {id}.{name} carried {} draws whole and {} divided into {shares}",
                expected.len(),
                found.len()
            );
            for (index, (expected, found)) in expected.iter().zip(found).enumerate() {
                let scale = expected.abs().max(found.abs()).max(1.0);
                let apart = (expected - found).abs() / scale;
                assert!(
                    apart <= settling(config) || (expected.is_nan() && found.is_nan()),
                    "{example}: {id}.{name} draw {index} was {expected} whole \
                     and {found} divided into {shares}, {apart} apart"
                );
            }
        }
    }
}

/// Solved in one piece, which is what dividing has to reproduce.
fn steady() -> EvaluationConfig {
    EvaluationConfig {
        seed: 0,
        sample_count: 1_000,
        shares: 1,
        ..EvaluationConfig::default()
    }
}

/// Solved hard enough that what is compared is the fixed point itself.
fn settled() -> EvaluationConfig {
    EvaluationConfig {
        tolerance: 1e-10,
        max_iterations: 20_000,
        ..steady()
    }
}

/// How far apart two runs of one design are entitled to be.
///
/// Stopping when movement falls below `tolerance` leaves the iterate a bounded
/// distance short of the fixed point, and that distance rather than the tolerance
/// is what separates two runs that stopped at different moments. Measured on the
/// shipped designs it stays within a few tens of tolerances; a thousand leaves
/// room without letting a real disagreement through, since a share reading the
/// wrong draws is wrong by whole percent rather than by parts per million.
fn settling(config: EvaluationConfig) -> f64 {
    1_000.0 * config.tolerance
}

/// Dividing the draws leaves every design's settled channels untouched.
#[test]
fn a_divided_solve_matches_an_undivided_one() {
    for example in SETTLING {
        for shares in [2, 3, 8] {
            agree(example, steady(), shares);
        }
    }
}

/// A bistable design still solves when divided, and still reports what it found.
///
/// Its draws are not required to land on the same branch an undivided solve put
/// them on: a share damps against its own worst draw, takes a different path, and
/// a design with more than one resting state can come to rest at the other one.
/// That is a property of the design. What must hold is that dividing does not
/// turn a solvable design into an unsolvable one, or invent values that are not
/// numbers.
#[test]
fn a_divided_solve_of_a_bistable_design_stays_on_a_branch() {
    for example in BISTABLE {
        let whole = solved(example, steady());
        let divided = solved(
            example,
            EvaluationConfig {
                shares: 4,
                ..steady()
            },
        );
        assert_eq!(
            whole.converged, divided.converged,
            "{example}: dividing changed whether it settled"
        );
        for (id, expected) in &whole.components {
            let found = &divided.components[id];
            for name in expected.channels.keys() {
                let draws = spread(&found.channels[name], steady().sample_count);
                assert!(
                    draws.iter().all(|draw| draw.is_finite()),
                    "{example}: {id}.{name} left the reals when divided"
                );
            }
        }
    }
}

/// Asked to settle harder, the shares and the whole agree more closely.
///
/// This is what says the two are converging on one fixed point rather than on
/// two nearby ones: the gap between them is how far each stopped short, and it
/// closes when they are told to stop later.
#[test]
fn shares_converge_on_the_same_fixed_point() {
    agree("checkout", settled(), 4);
}

/// A share that does not divide the ensemble evenly still reassembles exactly.
#[test]
fn an_uneven_division_reassembles_exactly() {
    agree(
        "checkout",
        EvaluationConfig {
            sample_count: 997,
            ..steady()
        },
        6,
    );
}

/// Advancing through time carries state between steps, so division has to hold
/// across the whole horizon rather than only at the moment being read.
#[test]
fn a_divided_transient_solve_matches_an_undivided_one() {
    agree(
        "checkout",
        EvaluationConfig {
            horizon: 12,
            mode: SolveMode::Transient,
            ..steady()
        },
        4,
    );
}

/// Asking for more shares than there are draws is not an error.
#[test]
fn dividing_further_than_there_are_draws_still_solves() {
    agree(
        "checkout",
        EvaluationConfig {
            sample_count: 4,
            ..steady()
        },
        8,
    );
}
