//! Behavioural coverage for the system-design prelude.
//!
//! These tests exercise the prelude the way a capacity model does: through
//! Squiggle source, with uncertain inputs, checking laws rather than fixed
//! numbers wherever a law exists. Sampled assertions use deterministic seeds and
//! tolerances tied to the standard error of the configured draw count.

use optimist::squiggle::{Distribution, Runtime, RuntimeConfig, Value};
use rstest::rstest;

const SAMPLE_COUNT: usize = 20_000;

fn runtime() -> Runtime {
    Runtime::with_config(RuntimeConfig {
        seed: 0x5157e_u64,
        sample_count: SAMPLE_COUNT,
        max_steps: 4_000_000,
    })
    .expect("runtime")
}

fn evaluate(source: &str) -> Value {
    runtime()
        .evaluate(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"))
}

fn number(source: &str) -> f64 {
    match evaluate(source) {
        Value::Number(value) => value,
        value => panic!("{source} produced {value:?}"),
    }
}

fn distribution(source: &str) -> Distribution {
    match evaluate(source) {
        Value::Distribution(value) => value,
        value => panic!("{source} produced {value:?}"),
    }
}

fn fails(source: &str) -> bool {
    runtime().evaluate(source).is_err()
}

/// Little's Law holds in every direction it can be read.
///
/// The three forms are one identity, so recovering any factor from the other two
/// must return the original quantity exactly.
#[rstest]
#[case("Little.occupancy(120, 0.25)", 30.0)]
#[case("Little.residence(30, 120)", 0.25)]
#[case("Little.rate(30, 0.25)", 120.0)]
fn littles_law_reads_in_every_direction(#[case] source: &str, #[case] expected: f64) {
    assert!((number(source) - expected).abs() < 1e-12, "{source}");
}

/// The law is distribution-free, so it survives uncertain inputs unchanged.
#[test]
fn littles_law_round_trips_through_uncertainty() {
    let recovered = distribution(
        "rate = lognormal(4, 0.3)\n\
         wait = uniform(0.1, 0.5)\n\
         occupancy = Little.occupancy(rate, wait)\n\
         Little.residence(occupancy, rate) - wait",
    );
    assert!(
        recovered.stdev().expect("stdev") < 1e-9,
        "recovered residence must cancel the original exactly"
    );
    assert!(recovered.mean().expect("mean").abs() < 1e-9);
}

/// Queueing delay is convex in utilisation, so uncertainty raises the mean.
///
/// This is the property a mean-only model destroys. Evaluating at mean
/// utilisation gives the delay at the centre; averaging the delay across the
/// distribution must exceed it, by Jensen's inequality.
#[test]
fn uncertain_utilisation_raises_mean_queueing_delay() {
    let at_the_mean = number("Queue.mm1Wait(0.01, 0.6)");
    let averaged = distribution("Queue.mm1Wait(0.01, uniform(0.3, 0.9))")
        .mean()
        .expect("mean");
    assert!(
        averaged > at_the_mean * 1.05,
        "expected convexity, got {averaged} against {at_the_mean}"
    );
}

/// An M/M/1 queue is the single-server case of the general result.
#[rstest]
#[case(0.1)]
#[case(0.5)]
#[case(0.85)]
fn one_server_agrees_between_the_general_and_special_case(#[case] utilisation: f64) {
    let single = number(&format!("Queue.mm1Wait(0.02, {utilisation})"));
    let general = number(&format!("Queue.mmcWait(0.02, 1, {utilisation})"));
    assert!(
        (single - general).abs() < 1e-9,
        "{utilisation}: {single} against {general}"
    );
}

/// Adding servers at fixed utilisation reduces waiting.
///
/// Pooling capacity is the classic economy of scale in queueing: the same
/// utilisation spread over more servers queues less.
#[test]
fn pooling_servers_reduces_waiting() {
    let waits = [1, 2, 4, 8, 16]
        .map(|servers| number(&format!("Queue.mmcWait(0.05, {servers}, 0.8)")))
        .to_vec();
    assert!(
        waits.windows(2).all(|pair| pair[1] < pair[0]),
        "waits must fall as servers are pooled, got {waits:?}"
    );
}

/// Saturation stays finite so a bottleneck is reported rather than thrown.
#[test]
fn saturated_queues_report_a_large_finite_delay() {
    let saturated = number("Queue.mm1Wait(0.01, 1.5)");
    assert!(saturated.is_finite());
    assert!(saturated > 1_000.0, "expected a saturation sentinel");
}

/// Retry amplification is the demand a policy adds to a failing dependency.
#[test]
fn retry_amplification_rises_as_a_dependency_fails() {
    let healthy = number("Reliability.retryAttempts(0.999, 3)");
    let degraded = number("Reliability.retryAttempts(0.5, 3)");
    let failing = number("Reliability.retryAttempts(0.01, 3)");
    assert!(healthy < 1.01, "a healthy dependency is called once");
    assert!(degraded > healthy && failing > degraded);
    assert!(failing < 3.0, "amplification is bounded by the budget");
}

/// Retrying raises success, and depth lowers it.
#[test]
fn retries_and_depth_move_reliability_in_opposite_directions() {
    let once = number("Reliability.retrySuccess(0.9, 1)");
    let thrice = number("Reliability.retrySuccess(0.9, 3)");
    assert!((once - 0.9).abs() < 1e-12);
    assert!((thrice - 0.999).abs() < 1e-12);

    let shallow = number("Reliability.serialSuccess(0.99, 1)");
    let deep = number("Reliability.serialSuccess(0.99, 64)");
    assert!(deep < shallow && deep < 0.53);
}

/// A one-step deadline race follows the exponential law exactly.
#[test]
fn a_single_step_deadline_race_is_exponential() {
    for deadline in [0.1_f64, 1.0, 4.0] {
        let received = number(&format!("Reliability.deadlineSuccess(1, 2, {deadline})"));
        let expected = 1.0 - (-deadline / 2.0f64).exp();
        assert!((received - expected).abs() < 1e-9, "{deadline}");
    }
}

/// Deadline success falls with depth and rises with the budget.
#[test]
fn deadline_success_responds_to_depth_and_budget() {
    let shallow = number("Reliability.deadlineSuccess(2, 0.1, 0.5)");
    let deep = number("Reliability.deadlineSuccess(8, 0.1, 0.5)");
    let generous = number("Reliability.deadlineSuccess(8, 0.1, 5)");
    assert!(deep < shallow);
    assert!(generous > deep);
    assert!((0.0..=1.0).contains(&deep));
}

/// An error budget is the failure count an objective permits over a window.
#[test]
fn an_error_budget_counts_permitted_failures() {
    // 1000 rps for an hour at 99.9% permits 3600 failures.
    assert!((number("Slo.errorBudget(1000, 0.999, 3600)") - 3600.0).abs() < 1e-6);
    assert!((number("Slo.errorBudget(1000, 0.99, 3600)") - 36_000.0).abs() < 1e-6);
    assert_eq!(number("Slo.errorBudget(1000, 1, 3600)"), 0.0);
}

/// Burn rate reports budget consumption as a window-independent multiple.
#[rstest]
#[case("Slo.burnRate(0.001, 0.999)", 1.0)]
#[case("Slo.burnRate(0.002, 0.999)", 2.0)]
#[case("Slo.burnRate(0.0005, 0.999)", 0.5)]
#[case("Slo.burnRate(0.01, 0.99)", 1.0)]
fn burn_rate_is_a_multiple_of_the_permitted_ratio(#[case] source: &str, #[case] expected: f64) {
    assert!((number(source) - expected).abs() < 1e-9, "{source}");
}

/// A perfect objective has no budget, so burning it is undefined rather than infinite.
#[test]
fn a_perfect_objective_has_no_budget_to_burn() {
    assert!(fails("Slo.burnRate(0.001, 1)"));
}

/// Saturation takes the extremum per draw, not per distribution.
///
/// Where demand and capacity overlap, some draws bind against capacity and
/// others do not. The result is a mixture whose mean sits strictly below both
/// inputs' means, which a comparison of summaries could not produce.
#[test]
fn saturation_clamps_each_draw_against_capacity() {
    let throughput = distribution("min([uniform(50, 150), 100])");
    assert!(throughput.maximum().expect("maximum") <= 100.0 + 1e-9);
    assert!(
        (throughput.mean().expect("mean") - 87.5).abs() < 1.0,
        "expected the mixture mean, got {}",
        throughput.mean().expect("mean")
    );

    let headroom = distribution("max([100 - uniform(50, 150), 0])");
    assert!(headroom.minimum().expect("minimum") >= -1e-9);
}

/// Saturation preserves dependence between the quantities being compared.
#[test]
fn saturation_respects_shared_inputs() {
    let bound = distribution("demand = uniform(10, 20)\nmin([demand, demand]) - demand");
    assert!(bound.stdev().expect("stdev") < 1e-12);
    assert!(bound.mean().expect("mean").abs() < 1e-12);
}

/// Clamping and truncation are different operations and must stay different.
///
/// Clamping moves excess draws onto the limit, leaving an atom whose mass is the
/// share of outcomes that saturate. Truncation removes them and renormalises
/// over what is left. A capacity model needs the first: substituting the second
/// would delete exactly the draws that evidence a bottleneck, and report a
/// healthy system precisely when demand had outgrown capacity.
#[test]
fn clamping_and_truncation_are_not_interchangeable() {
    let clamped = distribution("min([uniform(50, 150), 100])");
    let conditioned = distribution("truncateRight(uniform(50, 150), 100)");

    // Clamping keeps every draw, so half the mass lands exactly on the limit.
    let at_the_limit = clamped
        .samples()
        .expect("sample set")
        .iter()
        .filter(|draw| (**draw - 100.0).abs() < 1e-9)
        .count() as f64
        / clamped.samples().expect("sample set").len() as f64;
    assert!(
        (at_the_limit - 0.5).abs() < 0.02,
        "expected half the draws to saturate, got {at_the_limit}"
    );
    assert!((clamped.mean().expect("mean") - 87.5).abs() < 1.0);

    // Truncation discards them, leaving a uniform over the retained range.
    assert!((conditioned.mean().expect("mean") - 75.0).abs() < 1.0);
    assert!(conditioned.mean().expect("mean") < clamped.mean().expect("mean"));
}

/// Truncation keeps its draws aligned with the quantities it was derived from.
///
/// Conditioning is a monotone remap of the existing draws, so a truncated
/// quantity stays perfectly rank-correlated with its source. Resampling until
/// enough draws landed in range would have severed that link silently.
#[test]
fn truncation_preserves_dependence_on_its_source() {
    let source = distribution(
        "latency = lognormal(-3, 0.6)\n\
         within = truncateRight(latency, 0.2)\n\
         within - latency",
    );
    // A monotone remap can only move draws downward here, never reorder them.
    assert!(source.maximum().expect("maximum") <= 1e-9);

    let ranks = distribution(
        "latency = lognormal(-3, 0.6)\n\
         within = truncateRight(latency, 0.2)\n\
         min([within, latency]) - within",
    );
    assert!(
        ranks.stdev().expect("stdev") < 1e-12 && ranks.mean().expect("mean").abs() < 1e-12,
        "the truncated draw must remain the lesser of the aligned pair"
    );
}

/// Truncating to a bounded interval reproduces the analytic conditional mean.
#[test]
fn truncation_matches_the_analytical_conditional() {
    // Conditioning U(0, 10) on [2, 6] gives U(2, 6), whose mean is 4.
    let conditioned = distribution("truncate(uniform(0, 10), 2, 6)");
    assert!((conditioned.mean().expect("mean") - 4.0).abs() < 0.05);
    assert!(conditioned.minimum().expect("minimum") >= 2.0 - 1e-9);
    assert!(conditioned.maximum().expect("maximum") <= 6.0 + 1e-9);
}

/// An interval outside the support is rejected rather than looping or emptying.
#[test]
fn truncation_without_retained_mass_is_rejected() {
    assert!(fails("truncate(uniform(0, 1), 5, 6)"));
    assert!(fails("truncate(uniform(0, 1), 0.9, 0.1)"));
}

/// The prelude composes into the model project B had to hand-write.
///
/// Retries amplify demand against a dependency, that demand drives utilisation,
/// utilisation sets queueing delay, and delay decides whether a request meets its
/// deadline. Every quantity stays a distribution the whole way through, so the
/// answer reflects the spread of the inputs rather than their centres.
#[test]
fn a_retry_and_deadline_model_composes_end_to_end() {
    let success = distribution(
        "offered :: rps = 100\n\
         depth :: op = 8\n\
         attemptSuccess = 0.998\n\
         amplification = Reliability.retryAttempts(attemptSuccess, 3)\n\
         demand :: rps = offered * depth * amplification\n\
         serviceTime :: s = lognormal(-4, 0.4)\n\
         capacity :: rps = 220 / serviceTime\n\
         utilisation = Queue.utilisation(demand, capacity)\n\
         wait :: s = Queue.mm1Wait(serviceTime, utilisation)\n\
         residence :: s = wait + serviceTime\n\
         Reliability.deadlineSuccess(depth, residence, 2)",
    );
    let mean = success.mean().expect("mean");
    assert!(
        (0.0..=1.0).contains(&mean),
        "success must be a probability, got {mean}"
    );
    assert!(
        success.stdev().expect("stdev") > 1e-6,
        "uncertainty in service time must reach the outcome"
    );
    assert!(
        success.minimum().expect("minimum") >= 0.0
            && success.maximum().expect("maximum") <= 1.0 + 1e-9
    );
}

/// Domain violations are reported rather than silently producing a number.
#[rstest]
#[case::negative_probability("Reliability.retrySuccess(-0.1, 2)")]
#[case::probability_above_one("Reliability.serialSuccess(1.2, 2)")]
#[case::no_attempts("Reliability.retryAttempts(0.5, 0)")]
#[case::zero_depth("Reliability.deadlineSuccess(0, 1, 1)")]
#[case::zero_service("Reliability.deadlineSuccess(2, 0, 1)")]
#[case::zero_capacity("Queue.utilisation(10, 0)")]
#[case::no_servers("Queue.mmcWait(0.1, 0, 0.5)")]
#[case::objective_above_one("Slo.errorBudget(100, 1.5, 60)")]
fn invalid_inputs_are_rejected(#[case] source: &str) {
    assert!(fails(source), "{source} should have been rejected");
}
