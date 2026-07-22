//! Statistical laws derived from Squiggle's normative invariants document and
//! `packages/squiggle-lang/__tests__/dists/Invariants/*`.
//!
//! The upstream document is explicitly aspirational. This suite enforces valid
//! probability laws while avoiding its incorrect claim that a density must be at
//! most one. Runtime algebra is Monte Carlo, so sampled assertions use deterministic
//! seeds and tolerances tied to the standard error of the configured draw count.
//!
//! The PDF convolution law is not asserted for composed runtime distributions:
//! this sidecar currently stores compositions as empirical draws and its public
//! empirical `pdf` is discrete mass, not a KDE. CDF and moment laws still provide
//! coverage of the composed distributions.

use optimist::squiggle::{Distribution, Runtime, RuntimeConfig};
use rstest::rstest;

const SAMPLE_COUNT: usize = 50_000;

fn runtime() -> Result<Runtime, String> {
    Runtime::with_config(RuntimeConfig {
        seed: 0x5eed,
        sample_count: SAMPLE_COUNT,
        max_steps: 2_000_000,
    })
}

fn evaluate_distribution(source: &str) -> Result<Distribution, String> {
    let value = runtime()?
        .evaluate(source)
        .map_err(|diagnostics| format!("{diagnostics:?}"))?;
    value
        .as_distribution()
        .cloned()
        .ok_or_else(|| format!("{source} did not evaluate to a Distribution"))
}

#[rstest]
#[case::normal(Distribution::normal(2.0, 1.5))]
#[case::lognormal(Distribution::lognormal(0.5, 0.7))]
#[case::uniform(Distribution::uniform(-3.0, 7.0))]
#[case::beta(Distribution::beta(2.0, 5.0))]
#[case::cauchy(Distribution::cauchy(1.0, 2.0))]
#[case::exponential(Distribution::exponential(1.3))]
#[case::gamma(Distribution::gamma(3.0, 2.0))]
#[case::logistic(Distribution::logistic(-1.0, 0.8))]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0))]
fn cdf_and_quantile_are_inverses(
    #[case] distribution: Result<Distribution, String>,
) -> Result<(), String> {
    let distribution = distribution?;
    for probability in [0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99] {
        let quantile = distribution.quantile(probability)?;
        let recovered = distribution.cdf(quantile)?;
        assert!(
            (recovered - probability).abs() <= 1e-4,
            "{}: cdf(quantile({probability}))={recovered}",
            distribution.family()
        );
    }
    Ok(())
}

#[rstest]
#[case::normal(Distribution::normal(2.0, 1.5))]
#[case::lognormal(Distribution::lognormal(0.5, 0.7))]
#[case::uniform(Distribution::uniform(-3.0, 7.0))]
#[case::beta(Distribution::beta(2.0, 5.0))]
#[case::cauchy(Distribution::cauchy(1.0, 2.0))]
#[case::exponential(Distribution::exponential(1.3))]
#[case::gamma(Distribution::gamma(3.0, 2.0))]
#[case::logistic(Distribution::logistic(-1.0, 0.8))]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0))]
fn cdf_quantile_and_pdf_obey_order_and_range_laws(
    #[case] distribution: Result<Distribution, String>,
) -> Result<(), String> {
    let distribution = distribution?;
    let mut previous_quantile = f64::NEG_INFINITY;
    let mut previous_cdf = 0.0;
    for probability in [0.001, 0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999] {
        let quantile = distribution.quantile(probability)?;
        let cdf = distribution.cdf(quantile)?;
        let density = distribution.pdf(quantile)?;
        assert!(
            quantile >= previous_quantile,
            "{} quantiles are not monotone",
            distribution.family()
        );
        assert!(
            cdf >= previous_cdf,
            "{} CDF is not monotone",
            distribution.family()
        );
        assert!(
            (0.0..=1.0).contains(&cdf),
            "{} CDF={cdf}",
            distribution.family()
        );
        assert!(
            density >= 0.0 && density.is_finite(),
            "{} PDF={density}",
            distribution.family()
        );
        previous_quantile = quantile;
        previous_cdf = cdf;
    }
    Ok(())
}

#[rstest]
#[case::normal(Distribution::normal(2.0, 1.5), 1.0)]
#[case::lognormal(Distribution::lognormal(0.5, 0.7), 1.5)]
#[case::uniform(Distribution::uniform(-3.0, 7.0), 2.0)]
#[case::beta(Distribution::beta(2.0, 5.0), 0.3)]
#[case::cauchy(Distribution::cauchy(1.0, 2.0), 0.0)]
#[case::exponential(Distribution::exponential(1.3), 0.7)]
#[case::gamma(Distribution::gamma(3.0, 2.0), 4.0)]
#[case::logistic(Distribution::logistic(-1.0, 0.8), -0.5)]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0), 3.0)]
fn quantile_and_cdf_are_inverses_on_interior_support(
    #[case] distribution: Result<Distribution, String>,
    #[case] value: f64,
) -> Result<(), String> {
    let distribution = distribution?;
    let probability = distribution.cdf(value)?;
    let recovered = distribution.quantile(probability)?;
    assert!(
        (recovered - value).abs() <= 2e-4 * value.abs().max(1.0),
        "{}: quantile(cdf({value}))={recovered}",
        distribution.family()
    );
    Ok(())
}

#[rstest]
#[case::normal(Distribution::normal(2.0, 1.5), 2.0)]
#[case::lognormal(Distribution::lognormal(0.5, 0.7), 1.5)]
#[case::uniform(Distribution::uniform(-3.0, 7.0), 2.0)]
#[case::beta(Distribution::beta(2.0, 5.0), 0.3)]
#[case::cauchy(Distribution::cauchy(1.0, 2.0), 0.0)]
#[case::exponential(Distribution::exponential(1.3), 0.7)]
#[case::gamma(Distribution::gamma(3.0, 2.0), 4.0)]
#[case::logistic(Distribution::logistic(-1.0, 0.8), -0.5)]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0), 3.0)]
fn pdf_is_the_derivative_of_cdf(
    #[case] distribution: Result<Distribution, String>,
    #[case] value: f64,
) -> Result<(), String> {
    let distribution = distribution?;
    let step = 1e-5 * value.abs().max(1.0);
    let derivative =
        (distribution.cdf(value + step)? - distribution.cdf(value - step)?) / (2.0 * step);
    let density = distribution.pdf(value)?;
    let tolerance = 2e-4 * density.abs().max(1.0);
    assert!(
        (derivative - density).abs() <= tolerance,
        "{} at {value}: derivative={derivative}, pdf={density}, tolerance={tolerance}",
        distribution.family()
    );
    Ok(())
}

#[rstest]
#[case::normal(Distribution::normal(2.0, 1.5))]
#[case::lognormal(Distribution::lognormal(0.5, 0.7))]
#[case::uniform(Distribution::uniform(-3.0, 7.0))]
#[case::beta(Distribution::beta(2.0, 5.0))]
#[case::exponential(Distribution::exponential(1.3))]
#[case::gamma(Distribution::gamma(3.0, 2.0))]
#[case::logistic(Distribution::logistic(-1.0, 0.8))]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0))]
fn continuous_pdf_integrates_to_one(
    #[case] distribution: Result<Distribution, String>,
) -> Result<(), String> {
    let distribution = distribution?;
    let lower = distribution.quantile(1e-5)?;
    let upper = distribution.quantile(1.0 - 1e-5)?;
    let steps = 20_000;
    let width = (upper - lower) / steps as f64;
    let mut integral = 0.0;
    let mut previous = distribution.pdf(lower)?;
    for index in 1..=steps {
        let current = distribution.pdf(lower + index as f64 * width)?;
        integral += (previous + current) * width / 2.0;
        previous = current;
    }
    assert!(
        (integral - 1.0).abs() <= 3e-3,
        "{} density integrated to {integral}",
        distribution.family()
    );
    Ok(())
}

#[rstest]
#[case::point(Distribution::point(4.0), 4.0, 0.0)]
#[case::normal(Distribution::normal(4.0, 3.0), 4.0, 9.0)]
#[case::lognormal(Distribution::lognormal(0.5, 0.7), (0.5_f64 + 0.7_f64.powi(2) / 2.0).exp(), 0.7_f64.powi(2).exp_m1() * (1.0_f64 + 0.7_f64.powi(2)).exp())]
#[case::uniform(Distribution::uniform(-3.0, 7.0), 2.0, 100.0 / 12.0)]
#[case::beta(Distribution::beta(2.0, 5.0), 2.0 / 7.0, 10.0 / (49.0 * 8.0))]
#[case::bernoulli(Distribution::bernoulli(0.2), 0.2, 0.16)]
#[case::binomial(Distribution::binomial(10, 0.3), 3.0, 2.1)]
#[case::exponential(Distribution::exponential(2.0), 0.5, 0.25)]
#[case::gamma(Distribution::gamma(3.0, 2.0), 6.0, 12.0)]
#[case::logistic(Distribution::logistic(5.0, 1.0), 5.0, std::f64::consts::PI.powi(2) / 3.0)]
#[case::poisson(Distribution::poisson(4.0), 4.0, 4.0)]
#[case::triangular(Distribution::triangular(0.0, 2.0, 7.0), 3.0, 39.0 / 18.0)]
fn symbolic_moments_match_parameterizations(
    #[case] distribution: Result<Distribution, String>,
    #[case] expected_mean: f64,
    #[case] expected_variance: f64,
) -> Result<(), String> {
    let distribution = distribution?;
    assert!((distribution.mean()? - expected_mean).abs() <= 1e-12);
    assert!((distribution.variance()? - expected_variance).abs() <= 1e-12);
    Ok(())
}

#[rstest]
#[case::bernoulli(Distribution::bernoulli(0.3), 1)]
#[case::binomial(Distribution::binomial(12, 0.4), 12)]
#[case::poisson(Distribution::poisson(4.0), 40)]
fn discrete_probability_mass_normalizes(
    #[case] distribution: Result<Distribution, String>,
    #[case] maximum: u64,
) -> Result<(), String> {
    let distribution = distribution?;
    let mass = (0..=maximum)
        .map(|value| distribution.pdf(value as f64))
        .sum::<Result<f64, _>>()?;
    assert!(
        (mass - 1.0).abs() <= 1e-8,
        "{} mass={mass}",
        distribution.family()
    );
    Ok(())
}

#[test]
fn density_is_nonnegative_but_not_probability_bounded() -> Result<(), String> {
    let narrow_uniform = Distribution::uniform(0.0, 0.1)?;
    assert_eq!(narrow_uniform.pdf(0.05)?, 10.0);
    for index in 0..=100 {
        assert!(narrow_uniform.pdf(index as f64 / 1_000.0)? >= 0.0);
    }
    Ok(())
}

#[rstest]
#[case::normal_sum("normal(5,2)+normal(10,3)", 15.0, 13.0_f64.sqrt())]
#[case::normal_subtraction("normal(5,2)-normal(10,3)", -5.0, 13.0_f64.sqrt())]
#[case::normal_product("normal(5,2)*normal(10,3)", 50.0, 661.0_f64.sqrt())]
#[case::uniform_beta_sum(
    "uniform(9,10)+beta(2,5)",
    9.5 + 2.0 / 7.0,
    (1.0_f64 / 12.0 + 10.0 / (49.0 * 8.0)).sqrt()
)]
#[case::positive_scalar_product("normal(10,2)*2", 20.0, 4.0)]
#[case::positive_scalar_division("normal(10,2)/2", 5.0, 1.0)]
fn algebraic_mean_and_stdev_invariants(
    #[case] source: &str,
    #[case] expected_mean: f64,
    #[case] expected_stdev: f64,
) -> Result<(), String> {
    let distribution = evaluate_distribution(source)?;
    let received_mean = distribution.mean()?;
    let received_stdev = distribution.stdev()?;
    let mean_tolerance = 6.0 * expected_stdev / (SAMPLE_COUNT as f64).sqrt() + 1e-3;
    let stdev_tolerance = expected_stdev * 0.04 + 1e-3;
    assert!(
        (received_mean - expected_mean).abs() <= mean_tolerance,
        "{source}: mean expected {expected_mean}, received {received_mean}, tolerance {mean_tolerance}"
    );
    assert!(
        (received_stdev - expected_stdev).abs() <= stdev_tolerance,
        "{source}: stdev expected {expected_stdev}, received {received_stdev}, tolerance {stdev_tolerance}"
    );
    Ok(())
}

#[rstest]
#[case::lower_tail(10.0)]
#[case::center(15.0)]
#[case::upper_tail(20.0)]
fn normal_sum_cdf_matches_convolution(#[case] value: f64) -> Result<(), String> {
    let sampled = evaluate_distribution("normal(5,2)+normal(10,3)")?;
    let analytical = Distribution::normal(15.0, 13.0_f64.sqrt())?;
    let expected = analytical.cdf(value)?;
    let received = sampled.cdf(value)?;
    let standard_error = (expected * (1.0 - expected) / SAMPLE_COUNT as f64).sqrt();
    let tolerance = 6.0 * standard_error + 2e-3;
    assert!(
        (received - expected).abs() <= tolerance,
        "cdf at {value}: expected {expected}, received {received}, tolerance {tolerance}"
    );
    Ok(())
}
