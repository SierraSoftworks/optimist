use rand::Rng;

use crate::squiggle::{Diagnostic, Distribution, Value, ast::Span};

use super::{
    Runtime,
    builtin::{arity, number},
};

builtins! {
    context(runtime, span);
        "Dist.normal" | "Sym.normal" | normal(mean: Number, stdev: Number) => finish(Distribution::normal(mean, stdev), span),
        "Dist.normal" | "Sym.normal" | normal(values: Dictionary) => normal(vec![Value::Dictionary(values.clone())], span),
        "Dist.lognormal" | "Sym.lognormal" | lognormal(mu: Number, sigma: Number) => finish(Distribution::lognormal(mu, sigma), span),
        "Dist.lognormal" | "Sym.lognormal" | lognormal(values: Dictionary) => lognormal(vec![Value::Dictionary(values.clone())], span),
        "Dist.uniform" | "Sym.uniform" | uniform(low: Number, high: Number) => finish(Distribution::uniform(low, high), span),
        "Dist.beta" | "Sym.beta" | beta(alpha: Number, beta: Number) => finish(Distribution::beta(alpha, beta), span),
        "Dist.beta" | "Sym.beta" | beta(values: Dictionary) => beta(vec![Value::Dictionary(values.clone())], span),
        "Dist.cauchy" | "Sym.cauchy" | cauchy(location: Number, scale: Number) => finish(Distribution::cauchy(location, scale), span),
        "Dist.gamma" | "Sym.gamma" | gamma(shape: Number, scale: Number) => finish(Distribution::gamma(shape, scale), span),
        "Dist.logistic" | "Sym.logistic" | logistic(location: Number, scale: Number) => finish(Distribution::logistic(location, scale), span),
        "Dist.exponential" | "Sym.exponential" | exponential(rate: Number) => finish(Distribution::exponential(rate), span),
        "Dist.bernoulli" | "Sym.bernoulli" | bernoulli(probability: Number) => finish(Distribution::bernoulli(probability), span),
        "Dist.binomial" | "Sym.binomial" | binomial(trials: NonNegativeInteger, probability: Number) => finish(Distribution::binomial(trials, probability), span),
        "Dist.poisson" | "Sym.poisson" | poisson(rate: Number) => finish(Distribution::poisson(rate), span),
        "Dist.triangular" | "Sym.triangular" | triangular(low: Number, mode: Number, high: Number) => finish(Distribution::triangular(low, mode, high), span),
        "Sym.pointMass" | pointMass(value: Number) => finish(Distribution::point(value), span),
        "Dist.make" | make(value: Number) => finish(Distribution::point(value), span),
        "Dist.make" | make(value: Distribution) => Ok(Value::Distribution(value.clone())),
        "Dist.cdf" | cdf(distribution: Distribution, value: Number) => probability_operation("cdf", vec![Value::Distribution(distribution.clone()), Value::Number(value)], span),
        "Dist.pdf" | pdf(distribution: Distribution, value: Number) => probability_operation("pdf", vec![Value::Distribution(distribution.clone()), Value::Number(value)], span),
        "Dist.inv" | inv(distribution: Distribution, probability: Number) => probability_operation("inv", vec![Value::Distribution(distribution.clone()), Value::Number(probability)], span),
        "Dist.sample" | sample(distribution: Distribution) => sample(runtime, vec![Value::Distribution(distribution.clone())], span),
        "Dist.sampleN" | sampleN(distribution: Distribution, count: NonNegativeInteger) => sample_n(runtime, vec![Value::Distribution(distribution.clone()), Value::Number(count as f64)], span),
        "Dist.truncate" | truncate(distribution: Distribution, left: Number, right: Number) => truncate(runtime, "truncate", vec![Value::Distribution(distribution.clone()), Value::Number(left), Value::Number(right)], span),
        "Dist.truncateLeft" | truncateLeft(distribution: Distribution, left: Number) => truncate(runtime, "truncateLeft", vec![Value::Distribution(distribution.clone()), Value::Number(left)], span),
        "Dist.truncateRight" | truncateRight(distribution: Distribution, right: Number) => truncate(runtime, "truncateRight", vec![Value::Distribution(distribution.clone()), Value::Number(right)], span),
        "Dist.mixture" | mixture | mx(components: Array) => {
            mixture(runtime, vec![Value::Array(components.clone())], span)
        },
        "Dist.mixture" | mixture | mx(components: Array, weights: Array) => {
            mixture(runtime, vec![Value::Array(components.clone()), Value::Array(weights.clone())], span)
        },
    "Dist.mixture" | mixture | mx(first: *, ...rest: *) => {
        let mut components = Vec::with_capacity(rest.len() + 1);
        components.push(first.clone());
        components.extend_from_slice(rest);
        mixture(runtime, components, span)
    },
}

fn two(
    arguments: Vec<Value>,
    span: Span,
    constructor: fn(f64, f64) -> Result<Distribution, String>,
) -> Result<Value, Diagnostic> {
    arity(&arguments, 2, span)?;
    finish(
        constructor(number(&arguments[0], span)?, number(&arguments[1], span)?),
        span,
    )
}

fn normal(arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    if let [Value::Dictionary(values)] = arguments.as_slice() {
        if let (Some(mean), Some(stdev)) = (field(values, "mean"), field(values, "stdev")) {
            return finish(Distribution::normal(mean, stdev), span);
        }
        let (low, high, probability) = interval(values, span)?;
        let z = Distribution::normal(0.0, 1.0)
            .and_then(|distribution| distribution.quantile((1.0 + probability) / 2.0))
            .map_err(|error| Diagnostic::runtime(error, span))?;
        return finish(
            Distribution::normal((low + high) / 2.0, (high - low) / (2.0 * z)),
            span,
        );
    }
    two(arguments, span, Distribution::normal)
}

fn lognormal(arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    if let [Value::Dictionary(values)] = arguments.as_slice() {
        if let (Some(mean), Some(stdev)) = (field(values, "mean"), field(values, "stdev")) {
            let sigma2 = (1.0 + stdev * stdev / (mean * mean)).ln();
            return finish(
                Distribution::lognormal(mean.ln() - sigma2 / 2.0, sigma2.sqrt()),
                span,
            );
        }
        let (low, high, probability) = interval(values, span)?;
        if low <= 0.0 {
            return Err(Diagnostic::runtime(
                "Lognormal interval values must be positive",
                span,
            ));
        }
        let z = Distribution::normal(0.0, 1.0)
            .and_then(|distribution| distribution.quantile((1.0 + probability) / 2.0))
            .map_err(|error| Diagnostic::runtime(error, span))?;
        return finish(
            Distribution::lognormal(
                (low.ln() + high.ln()) / 2.0,
                (high.ln() - low.ln()) / (2.0 * z),
            ),
            span,
        );
    }
    two(arguments, span, Distribution::lognormal)
}

fn beta(arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    if let [Value::Dictionary(values)] = arguments.as_slice() {
        let (Some(mean), Some(stdev)) = (field(values, "mean"), field(values, "stdev")) else {
            return Err(Diagnostic::runtime(
                "beta dictionary requires mean and stdev",
                span,
            ));
        };
        let common = mean * (1.0 - mean) / stdev.powi(2) - 1.0;
        return finish(
            Distribution::beta(mean * common, (1.0 - mean) * common),
            span,
        );
    }
    two(arguments, span, Distribution::beta)
}

fn probability_operation(
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(&arguments, 2, span)?;
    let distribution = distribution(&arguments[0], span)?;
    let value = number(&arguments[1], span)?;
    let result = match name {
        "cdf" => distribution.cdf(value),
        "pdf" => distribution.pdf(value),
        _ => distribution.quantile(value),
    }
    .map_err(|error| Diagnostic::runtime(error, span))?;
    Ok(Value::Number(result))
}

fn sample(runtime: &mut Runtime, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    arity(&arguments, 1, span)?;
    let value = distribution(&arguments[0], span)?
        .sample(&mut runtime.rng)
        .map_err(|error| Diagnostic::runtime(error, span))?;
    Ok(Value::Number(value))
}

fn sample_n(runtime: &mut Runtime, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    arity(&arguments, 2, span)?;
    let distribution = distribution(&arguments[0], span)?;
    let count = integer(&arguments[1], span)?;
    let samples = distribution
        .sample_n(count, &mut runtime.rng)
        .map_err(|error| Diagnostic::runtime(error, span))?;
    Ok(Value::Array(
        samples.into_iter().map(Value::Number).collect(),
    ))
}

fn mixture(runtime: &mut Runtime, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    let (components, weights) = match arguments.as_slice() {
        [Value::Array(components)] => (components.clone(), vec![1.0; components.len()]),
        [Value::Array(components), Value::Array(weights)] => (
            components.clone(),
            weights
                .iter()
                .map(|value| number(value, span))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => (arguments.clone(), vec![1.0; arguments.len()]),
    };
    if components.is_empty()
        || components.len() != weights.len()
        || weights.iter().any(|weight| *weight < 0.0)
    {
        return Err(Diagnostic::runtime(
            "mixture requires components and matching non-negative weights",
            span,
        ));
    }
    let total = weights.iter().sum::<f64>();
    if total <= 0.0 {
        return Err(Diagnostic::runtime(
            "mixture weights must have a positive sum",
            span,
        ));
    }
    let mut samples = Vec::with_capacity(runtime.config.sample_count);
    for _ in 0..runtime.config.sample_count {
        let mut pick = runtime.rng.gen_range(0.0..total);
        let mut index = weights.len() - 1;
        for (candidate, weight) in weights.iter().enumerate() {
            pick -= weight;
            if pick <= 0.0 {
                index = candidate;
                break;
            }
        }
        samples.push(match &components[index] {
            Value::Number(value) => *value,
            value => distribution(value, span)?
                .sample(&mut runtime.rng)
                .map_err(|error| Diagnostic::runtime(error, span))?,
        });
    }
    finish(Distribution::from_samples(samples), span)
}

/// Conditions a distribution on falling inside an interval.
///
/// Truncation is conditioning, not clamping. The result is the law of
/// $X \mid l \leq X \leq r$, whose distribution function is renormalised over
/// the retained mass:
///
/// $$F_{Y}(y) = \frac{F_X(y) - F_X(l)}{F_X(r) - F_X(l)}, \qquad l \leq y \leq r$$
///
/// Draws outside the interval are therefore *removed* rather than moved to the
/// boundary. Where a limit represents a capacity that demand piles up against,
/// `min` and `max` are the correct operations, because the mass they leave at the
/// boundary is exactly the share of outcomes that saturate. Truncation is right
/// where the limit selects a subpopulation instead, such as the latency of calls
/// that returned before their timeout expired.
///
/// # Preserving dependence
///
/// The interval is applied by remapping each existing draw through the truncated
/// quantile function rather than by resampling until enough draws land in range:
///
/// $$y_i = F_X^{-1}\bigl(F_X(l) + F_X(x_i)\,[F_X(r) - F_X(l)]\bigr)$$
///
/// Because $F_X$ is non-decreasing, this is a monotone transform of the original
/// draws, so the result keeps its index alignment and its dependence on every
/// quantity upstream. Rejection sampling would have returned draws with no
/// correspondence to the ones they replaced, silently severing a model's
/// correlation structure. The remap is also exact and terminates, where rejection
/// sampling degrades as the retained mass shrinks and eventually fails outright.
///
/// Recovering a draw's position as $F_X(x_i)$ is exact for continuous families.
/// A discrete family maps a whole interval of positions onto each atom, so the
/// recovered position is the top of that interval and a truncated discrete
/// distribution is biased slightly toward its upper atoms.
fn truncate(
    runtime: &mut Runtime,
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let expected = if name == "truncate" { 3 } else { 2 };
    arity(&arguments, expected, span)?;
    let distribution = distribution(&arguments[0], span)?;
    let (left, right) = match name {
        "truncate" => (number(&arguments[1], span)?, number(&arguments[2], span)?),
        "truncateLeft" => (number(&arguments[1], span)?, f64::INFINITY),
        _ => (f64::NEG_INFINITY, number(&arguments[1], span)?),
    };
    if left > right {
        return Err(Diagnostic::runtime(
            "truncation requires a lower bound below its upper bound",
            span,
        ));
    }
    let fail = |error: String| Diagnostic::runtime(error, span);
    let lower = if left.is_infinite() {
        0.0
    } else {
        distribution.cdf(left).map_err(fail)?
    };
    let upper = if right.is_infinite() {
        1.0
    } else {
        distribution.cdf(right).map_err(fail)?
    };
    let retained = upper - lower;
    if retained <= f64::EPSILON {
        return Err(
            Diagnostic::runtime("truncation interval retains no probability mass", span)
                .with_help("widen the interval so it overlaps the distribution's support"),
        );
    }
    let count = Distribution::aligned([distribution], runtime.ensemble);
    let samples = distribution
        .materialise(distribution.stream(&mut runtime.rng), count)
        .into_iter()
        .map(|draw| {
            let position = distribution.cdf(draw)?;
            distribution.quantile(lower + position * retained)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(fail)?;
    finish(Distribution::from_samples(samples), span)
}

fn interval(
    values: &std::collections::BTreeMap<String, Value>,
    span: Span,
) -> Result<(f64, f64, f64), Diagnostic> {
    for (low, high, probability) in [("p5", "p95", 0.9), ("p10", "p90", 0.8), ("p25", "p75", 0.5)] {
        if let (Some(low), Some(high)) = (field(values, low), field(values, high)) {
            return Ok((low, high, probability));
        }
    }
    Err(Diagnostic::runtime(
        "credible interval requires p5/p95, p10/p90, or p25/p75",
        span,
    ))
}

fn field(values: &std::collections::BTreeMap<String, Value>, name: &str) -> Option<f64> {
    values.get(name)?.as_number()
}
fn integer(value: &Value, span: Span) -> Result<usize, Diagnostic> {
    let value = number(value, span)?;
    if value < 0.0 || value.fract() != 0.0 {
        Err(Diagnostic::runtime("expected a non-negative integer", span))
    } else {
        Ok(value as usize)
    }
}
fn distribution(value: &Value, span: Span) -> Result<&Distribution, Diagnostic> {
    value.as_distribution().ok_or_else(|| {
        Diagnostic::runtime(
            format!("expected Distribution, received {}", value.type_name()),
            span,
        )
    })
}
fn finish(result: Result<Distribution, String>, span: Span) -> Result<Value, Diagnostic> {
    result
        .map(Value::Distribution)
        .map_err(|error| Diagnostic::runtime(error, span))
}
