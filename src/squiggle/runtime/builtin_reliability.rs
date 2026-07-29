//! Reliability composition and service level arithmetic.
//!
//! # Retries
//!
//! With attempt success probability $p$ and at most $n$ attempts, and assuming
//! attempts fail independently, the probability that a call eventually succeeds
//! is the complement of every attempt failing:
//!
//! $$P(\text{success}) = 1 - (1 - p)^n$$
//!
//! The expected number of attempts actually made matters more than the success
//! probability, because it is the amplification a retry policy applies to
//! downstream demand. Stopping at the first success or the $n$th attempt gives a
//! truncated geometric count with mean
//!
//! $$\mathbb{E}[N] = \sum_{k=1}^{n} (1-p)^{k-1} = \frac{1 - (1-p)^n}{p}$$
//!
//! which tends to $n$ as $p \to 0$. This is the term that turns a partial
//! outage into a retry storm: as $p$ falls, every caller multiplies its load on
//! the dependency that is already failing.
//!
//! Independence is the load-bearing assumption and it is optimistic. Attempts
//! against a saturated dependency fail together, so a model that needs
//! correlated failure must express it through shared upstream uncertainty rather
//! than relying on these formulas.
//!
//! # Serial dependencies
//!
//! A call that must complete $k$ independent steps succeeds with probability
//! $p^k$. Reliability falls geometrically in depth, which is why deep synchronous
//! call chains fail far more often than any single hop suggests.
//!
//! # Deadline races
//!
//! A request that performs $k$ sequential steps, each taking an exponential time
//! with mean $S$, finishes in the sum of $k$ exponentials. That sum is
//! Erlang-distributed with shape $k$ and rate $1/S$, so the probability of
//! finishing within a deadline $D$ is the regularised lower incomplete gamma
//! function
//!
//! $$P(k, D/S) = \frac{\gamma(k, D/S)}{\Gamma(k)}$$
//!
//! Exponential steps are assumed, which is the maximum-variability choice for a
//! given mean, so this is a conservative estimate of meeting a deadline. Shape is
//! taken as a continuous parameter so that a non-integer mean depth remains
//! expressible, which generalises the Erlang law to the gamma law.
//!
//! # Service levels
//!
//! An objective $o$ is the fraction of eligible operations required to succeed
//! over a window. The error budget is the number of failures the objective
//! permits in that window:
//!
//! $$\text{budget} = \lambda \, T \, (1 - o)$$
//!
//! Burn rate compares observed failure against that allowance as a multiple:
//! $\text{burn} = r / (1 - o)$ for an observed error ratio $r$. A burn rate of
//! one exhausts the budget exactly at the end of the window, two exhausts it at
//! the halfway point, and below one leaves budget unspent. Expressing burn as a
//! multiple rather than a count is what makes a single alerting threshold work
//! across windows of different lengths.
//!
//! References: Google, *Site Reliability Engineering* (2016), chapters 3 and 4 on
//! error budgets, and *The Site Reliability Workbook* (2018), chapter 5 on burn
//! rate alerting; Milton Abramowitz and Irene Stegun, *Handbook of Mathematical
//! Functions* (1964), section 6.5 for the incomplete gamma function.

use statrs::distribution::{ContinuousCDF, Gamma};

use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::{
    Runtime,
    elementwise::{elementwise, finite},
};

builtins! {
    context(runtime, span);
        "Reliability.retrySuccess"(attempt: (Number | Distribution), attempts: (Number | Distribution)) =>
            elementwise(runtime, &[attempt.clone(), attempts.clone()], span, |row| {
                let failure = 1.0 - probability(row[0], "attempt success")?;
                finite(1.0 - raised(failure, tries(row[1])?), "retry success probability")
            }),
        "Reliability.retryAttempts"(attempt: (Number | Distribution), attempts: (Number | Distribution)) =>
            elementwise(runtime, &[attempt.clone(), attempts.clone()], span, |row| {
                let success = probability(row[0], "attempt success")?;
                let tries = tries(row[1])?;
                let expected = if success <= f64::EPSILON {
                    tries
                } else {
                    (1.0 - raised(1.0 - success, tries)) / success
                };
                finite(expected, "expected attempts")
            }),
        "Reliability.serialSuccess"(step: (Number | Distribution), steps: (Number | Distribution)) =>
            elementwise(runtime, &[step.clone(), steps.clone()], span, |row| {
                let success = probability(row[0], "step success")?;
                finite(raised(success, depth(row[1])?), "serial success probability")
            }),
        "Reliability.deadlineSuccess"(steps: (Number | Distribution), service: (Number | Distribution), deadline: (Number | Distribution)) =>
            elementwise(runtime, &[steps.clone(), service.clone(), deadline.clone()], span, |row| {
                erlang_cdf(depth(row[0])?, positive(row[1], "service time")?, row[2])
            }),
        "Slo.errorBudget"(rate: (Number | Distribution), objective: (Number | Distribution), window: (Number | Distribution)) =>
            elementwise(runtime, &[rate.clone(), objective.clone(), window.clone()], span, |row| {
                let objective = probability(row[1], "objective")?;
                finite(row[0] * row[2] * (1.0 - objective), "error budget")
            }),
        "Slo.burnRate"(observed: (Number | Distribution), objective: (Number | Distribution)) =>
            elementwise(runtime, &[observed.clone(), objective.clone()], span, |row| {
                let allowed = 1.0 - probability(row[1], "objective")?;
                if allowed <= 0.0 {
                    return Err("a perfect objective leaves no budget to burn".to_owned());
                }
                finite(probability(row[0], "observed error ratio")? / allowed, "burn rate")
            }),
}

fn probability(value: f64, what: &str) -> Result<f64, String> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(format!("{what} must lie between zero and one"));
    }
    Ok(value)
}

fn positive(value: f64, what: &str) -> Result<f64, String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("{what} must be greater than zero"));
    }
    Ok(value)
}

fn tries(value: f64) -> Result<f64, String> {
    if !value.is_finite() || value < 1.0 {
        return Err("an attempt budget must allow at least one attempt".to_owned());
    }
    Ok(value)
}

fn depth(value: f64) -> Result<f64, String> {
    if !value.is_finite() || value <= 0.0 {
        return Err("a call depth must be greater than zero".to_owned());
    }
    Ok(value)
}

/// Raises a base to an exponent that is nearly always a whole number.
///
/// Attempt budgets and call depths are counts. `powf` evaluates
/// $e^{y \ln x}$ whether or not $y$ is whole, which costs several times what
/// repeated squaring does and, for the small counts a design writes, is no more
/// accurate: binary exponentiation of a handful of factors accumulates fewer
/// rounding errors than a logarithm and an exponential do. The bound keeps the
/// exponent inside `i32` and keeps the error of repeated squaring, which grows
/// with the number of factors, negligible against the logarithmic form.
fn raised(base: f64, exponent: f64) -> f64 {
    if exponent.fract() == 0.0 && (0.0..=1_024.0).contains(&exponent) {
        return base.powi(exponent as i32);
    }
    base.powf(exponent)
}

/// Probability that a chain of `steps` exponential stages finishes by `deadline`.
///
/// A request that makes `k` calls in series, each taking an exponentially
/// distributed time with mean `s`, finishes in the sum of those times, which is
/// Erlang-distributed with shape `k` and rate `1/s`. The chance it beats a
/// deadline `d` is that distribution's CDF, the regularised lower incomplete
/// gamma function $P(k, d/s)$.
///
/// For integer `k` — which is what a call depth is — that function has a closed
/// form rather than needing a series or continued fraction (Abramowitz and
/// Stegun, *Handbook of Mathematical Functions* (1964), equation 6.5.13):
///
/// $$P(k, x) = 1 - \sum_{n=0}^{k-1} \frac{x^n e^{-x}}{n!}, \qquad x = d/s$$
///
/// The summands are Poisson probabilities, and they are accumulated as such:
/// starting from $e^{-x}$ and carrying $t_n = t_{n-1} \cdot x / n$. Writing it
/// this way rather than summing $x^n/n!$ and multiplying by $e^{-x}$ at the end
/// is what keeps it usable across the whole domain — the bare series overflows
/// for $x \gtrsim 10^2$ while the factor it would be multiplied by has already
/// underflowed, producing `inf * 0`. Every term here lies in $[0, 1]$, no
/// factorial is formed, and all terms are positive so the sum does not cancel.
///
/// A single stage is the exponential CDF, evaluated as $-\mathrm{expm1}(-x)$ so
/// that a deadline far shorter than the service time keeps its significant
/// figures instead of losing them to $1 - (1 - \epsilon)$.
///
/// Accuracy is otherwise limited by the subtraction from one, so a near-certain
/// race resolves to about `k` machine epsilons absolute while a near-hopeless one
/// is exact to a relative epsilon.
///
/// Non-integer shapes are left to [`statrs`], which evaluates the general
/// incomplete gamma. They arise only when a design makes call depth itself
/// uncertain, which is off the path this exists to serve.
fn erlang_cdf(steps: f64, service: f64, deadline: f64) -> Result<f64, String> {
    if deadline <= 0.0 {
        return Ok(0.0);
    }
    let x = deadline / service;
    let Some(stages) = integer_stages(steps) else {
        return Gamma::new(steps, 1.0 / service)
            .map_err(|_| "the deadline race has no valid gamma parameters".to_owned())
            .map(|gamma| gamma.cdf(deadline).clamp(0.0, 1.0));
    };
    if stages == 1 {
        return Ok((-(-x).exp_m1()).clamp(0.0, 1.0));
    }
    let mut term = (-x).exp();
    let mut unfinished = term;
    for stage in 1..stages {
        term *= x / f64::from(stage);
        unfinished += term;
    }
    Ok((1.0 - unfinished).clamp(0.0, 1.0))
}

/// Reads a call depth as a whole number of stages, when it is one.
fn integer_stages(steps: f64) -> Option<u32> {
    (steps.fract() == 0.0 && (1.0..=1_000.0).contains(&steps)).then_some(steps as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closed form and the general incomplete gamma must agree, because which
    /// one a call takes is decided only by whether its depth happens to be whole.
    #[test]
    fn the_closed_form_agrees_with_the_general_incomplete_gamma() {
        for steps in [1.0_f64, 2.0, 3.0, 5.0, 12.0, 40.0] {
            for service in [1e-3_f64, 0.05, 1.0, 25.0] {
                for deadline in [1e-4_f64, 0.02, 0.5, 3.0, 100.0] {
                    let closed = erlang_cdf(steps, service, deadline).expect("closed form");
                    let general = Gamma::new(steps, 1.0 / service)
                        .expect("gamma")
                        .cdf(deadline)
                        .clamp(0.0, 1.0);
                    assert!(
                        (closed - general).abs() < 1e-9,
                        "k={steps} s={service} d={deadline}: {closed} vs {general}"
                    );
                }
            }
        }
    }

    /// A single stage is exponential, which is the one case with an elementary form.
    #[test]
    fn one_stage_is_the_exponential_distribution() {
        for (service, deadline) in [(1.0, 1.0), (0.25, 0.1), (10.0, 45.0)] {
            let expected: f64 = 1.0 - (-deadline / service as f64).exp();
            assert!((erlang_cdf(1.0, service, deadline).expect("cdf") - expected).abs() < 1e-12);
        }
    }

    /// A deadline race is a probability, and it has to behave like one at the edges.
    #[test]
    fn the_deadline_race_stays_a_probability() {
        for steps in [1.0_f64, 4.0, 37.0] {
            assert_eq!(erlang_cdf(steps, 1.0, 0.0).expect("cdf"), 0.0);
            assert_eq!(erlang_cdf(steps, 1.0, -1.0).expect("cdf"), 0.0);
            assert!(erlang_cdf(steps, 1.0, 1e12).expect("cdf") > 1.0 - 1e-12);
            assert!(erlang_cdf(steps, 1e12, 1.0).expect("cdf") < 1e-9);
        }
    }

    /// More work in series, or less time to do it in, can only lower the odds.
    #[test]
    fn the_deadline_race_is_monotone_in_depth_and_in_time() {
        let deeper: Vec<f64> = (1..=8)
            .map(|steps| erlang_cdf(f64::from(steps), 0.1, 0.5).expect("cdf"))
            .collect();
        assert!(deeper.windows(2).all(|pair| pair[1] < pair[0]));
        let longer: Vec<f64> = [0.1_f64, 0.2, 0.4, 0.8]
            .iter()
            .map(|deadline| erlang_cdf(3.0, 0.1, *deadline).expect("cdf"))
            .collect();
        assert!(longer.windows(2).all(|pair| pair[1] > pair[0]));
    }

    fn expected_attempts(success: f64, tries: f64) -> f64 {
        (1.0 - (1.0 - success).powf(tries)) / success
    }

    #[test]
    fn a_single_attempt_is_its_own_success_probability() {
        for success in [0.0_f64, 0.3, 1.0] {
            assert!((1.0 - (1.0 - success).powf(1.0) - success).abs() < 1e-12);
        }
    }

    #[test]
    fn retry_amplification_grows_as_attempts_fail() {
        // The demand a retry policy places downstream rises as success falls,
        // which is the mechanism behind a retry storm.
        let amplification = [0.9_f64, 0.5, 0.2, 0.05]
            .map(|success| expected_attempts(success, 3.0))
            .to_vec();
        assert!(amplification.windows(2).all(|pair| pair[1] > pair[0]));
        assert!(*amplification.last().expect("last") < 3.0);
    }

    #[test]
    fn amplification_approaches_the_attempt_budget_as_success_vanishes() {
        assert!((expected_attempts(1e-9, 4.0) - 4.0).abs() < 1e-3);
    }

    #[test]
    fn serial_reliability_falls_geometrically_with_depth() {
        let success: f64 = 0.99;
        assert!(success.powf(64.0) < success.powf(8.0));
        assert!((success.powf(8.0) - 0.922_744_694_427_920_5).abs() < 1e-12);
    }

    #[test]
    fn a_one_step_deadline_race_is_the_exponential_law() {
        // Erlang with shape one is exponential, so P(T <= D) = 1 - e^{-D/S}.
        for deadline in [0.1_f64, 1.0, 5.0] {
            let received = erlang_cdf(1.0, 2.0, deadline).expect("cdf");
            let expected = 1.0 - (-deadline / 2.0).exp();
            assert!((received - expected).abs() < 1e-9, "{deadline}");
        }
    }

    #[test]
    fn deadline_success_falls_as_steps_are_added() {
        let odds = (1..8)
            .map(|steps| erlang_cdf(steps as f64, 0.1, 0.5).expect("cdf"))
            .collect::<Vec<_>>();
        assert!(odds.windows(2).all(|pair| pair[1] < pair[0]));
    }

    #[test]
    fn deadline_success_is_a_probability_and_rises_with_the_budget() {
        let tight = erlang_cdf(4.0, 0.05, 0.1).expect("cdf");
        let generous = erlang_cdf(4.0, 0.05, 10.0).expect("cdf");
        assert!((0.0..=1.0).contains(&tight));
        assert!(generous > tight);
        assert!(generous > 0.999);
        assert_eq!(erlang_cdf(3.0, 1.0, -1.0).expect("cdf"), 0.0);
    }

    #[test]
    fn out_of_range_inputs_are_rejected() {
        assert!(probability(1.5, "objective").is_err());
        assert!(probability(f64::NAN, "objective").is_err());
        assert!(tries(0.0).is_err());
        assert!(depth(0.0).is_err());
        assert!(positive(0.0, "service time").is_err());
    }
}
