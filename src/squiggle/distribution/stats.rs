use statrs::distribution::{
    Bernoulli, Beta, Binomial, Cauchy, Continuous, ContinuousCDF, Discrete, DiscreteCDF, Exp,
    Gamma, LogNormal, Normal, Poisson, Triangular, Uniform,
};
use std::fmt;

use super::{Distribution, Kind};

impl Distribution {
    /// Returns the expected value when it exists.
    pub fn mean(&self) -> Result<f64, String> {
        match &self.kind {
            Kind::Point(value) => Ok(*value),
            Kind::Normal(mean, _) | Kind::Logistic(mean, _) => Ok(*mean),
            Kind::Lognormal(mu, sigma) => Ok((mu + sigma * sigma / 2.0).exp()),
            Kind::Uniform(low, high) => Ok((low + high) / 2.0),
            Kind::Beta(alpha, beta) => Ok(alpha / (alpha + beta)),
            Kind::Bernoulli(p) => Ok(*p),
            Kind::Binomial(n, p) => Ok(*n as f64 * p),
            Kind::Cauchy(..) => Err("the Cauchy distribution has no mean".into()),
            Kind::Exponential(rate) => Ok(1.0 / rate),
            Kind::Gamma(shape, scale) => Ok(shape * scale),
            Kind::Poisson(rate) => Ok(*rate),
            Kind::Triangular(low, mode, high) => Ok((low + mode + high) / 3.0),
            Kind::Samples(samples) => Ok(samples.iter().sum::<f64>() / samples.len() as f64),
        }
    }

    /// Returns the population variance when it exists.
    pub fn variance(&self) -> Result<f64, String> {
        match &self.kind {
            Kind::Point(_) => Ok(0.0),
            Kind::Normal(_, sigma) => Ok(sigma * sigma),
            Kind::Lognormal(mu, sigma) => {
                Ok((sigma * sigma).exp_m1() * (2.0 * mu + sigma * sigma).exp())
            }
            Kind::Uniform(low, high) => Ok((high - low).powi(2) / 12.0),
            Kind::Beta(alpha, beta) => {
                Ok(alpha * beta / ((alpha + beta).powi(2) * (alpha + beta + 1.0)))
            }
            Kind::Bernoulli(p) => Ok(p * (1.0 - p)),
            Kind::Binomial(n, p) => Ok(*n as f64 * p * (1.0 - p)),
            Kind::Cauchy(..) => Err("the Cauchy distribution has no variance".into()),
            Kind::Exponential(rate) => Ok(1.0 / (rate * rate)),
            Kind::Gamma(shape, scale) => Ok(shape * scale * scale),
            Kind::Logistic(_, scale) => Ok(std::f64::consts::PI.powi(2) * scale * scale / 3.0),
            Kind::Poisson(rate) => Ok(*rate),
            Kind::Triangular(low, mode, high) => Ok((low * low + mode * mode + high * high
                - low * mode
                - low * high
                - mode * high)
                / 18.0),
            Kind::Samples(samples) => {
                let mean = self.mean()?;
                Ok(samples
                    .iter()
                    .map(|value| (value - mean).powi(2))
                    .sum::<f64>()
                    / samples.len() as f64)
            }
        }
    }

    /// Returns the population standard deviation when it exists.
    pub fn stdev(&self) -> Result<f64, String> {
        self.variance().map(f64::sqrt)
    }

    /// Returns the inverse CDF at probability `p` in `[0, 1]`.
    pub fn quantile(&self, p: f64) -> Result<f64, String> {
        if !(0.0..=1.0).contains(&p) || p.is_nan() {
            return Err("quantile probability must be between zero and one".into());
        }
        match &self.kind {
            Kind::Point(value) => Ok(*value),
            Kind::Normal(mean, sigma) => {
                Ok(validated("Normal", Normal::new(*mean, *sigma))?.inverse_cdf(p))
            }
            Kind::Lognormal(mu, sigma) => {
                Ok(validated("Lognormal", LogNormal::new(*mu, *sigma))?.inverse_cdf(p))
            }
            Kind::Uniform(low, high) => {
                Ok(validated("Uniform", Uniform::new(*low, *high))?.inverse_cdf(p))
            }
            Kind::Beta(alpha, beta) => {
                Ok(validated("Beta", Beta::new(*alpha, *beta))?.inverse_cdf(p))
            }
            Kind::Bernoulli(value) => {
                Ok(validated("Bernoulli", Bernoulli::new(*value))?.inverse_cdf(p) as f64)
            }
            Kind::Binomial(n, value) => {
                Ok(validated("Binomial", Binomial::new(*value, *n))?.inverse_cdf(p) as f64)
            }
            Kind::Cauchy(location, scale) => {
                Ok(validated("Cauchy", Cauchy::new(*location, *scale))?.inverse_cdf(p))
            }
            Kind::Exponential(rate) => {
                Ok(validated("Exponential", Exp::new(*rate))?.inverse_cdf(p))
            }
            Kind::Gamma(shape, scale) => {
                Ok(validated("Gamma", Gamma::new(*shape, 1.0 / scale))?.inverse_cdf(p))
            }
            Kind::Logistic(location, scale) => Ok(location + scale * (p / (1.0 - p)).ln()),
            Kind::Poisson(rate) => {
                Ok(validated("Poisson", Poisson::new(*rate))?.inverse_cdf(p) as f64)
            }
            Kind::Triangular(low, mode, high) => {
                Ok(validated("Triangular", Triangular::new(*low, *high, *mode))?.inverse_cdf(p))
            }
            Kind::Samples(samples) => empirical_quantile(samples, p),
        }
    }

    /// Returns the cumulative probability at `x`.
    pub fn cdf(&self, x: f64) -> Result<f64, String> {
        if x.is_nan() {
            return Err("CDF input must not be NaN".into());
        }
        Ok(match &self.kind {
            Kind::Point(value) => f64::from(x >= *value),
            Kind::Normal(mean, sigma) => validated("Normal", Normal::new(*mean, *sigma))?.cdf(x),
            Kind::Lognormal(mu, sigma) => {
                validated("Lognormal", LogNormal::new(*mu, *sigma))?.cdf(x)
            }
            Kind::Uniform(low, high) => validated("Uniform", Uniform::new(*low, *high))?.cdf(x),
            Kind::Beta(alpha, beta) => validated("Beta", Beta::new(*alpha, *beta))?.cdf(x),
            Kind::Bernoulli(p) => {
                if x < 0.0 {
                    0.0
                } else {
                    validated("Bernoulli", Bernoulli::new(*p))?.cdf(x.floor() as u64)
                }
            }
            Kind::Binomial(n, p) => {
                if x < 0.0 {
                    0.0
                } else {
                    validated("Binomial", Binomial::new(*p, *n))?.cdf(x.floor() as u64)
                }
            }
            Kind::Cauchy(location, scale) => {
                validated("Cauchy", Cauchy::new(*location, *scale))?.cdf(x)
            }
            Kind::Exponential(rate) => validated("Exponential", Exp::new(*rate))?.cdf(x),
            Kind::Gamma(shape, scale) => {
                validated("Gamma", Gamma::new(*shape, 1.0 / scale))?.cdf(x)
            }
            Kind::Logistic(location, scale) => 1.0 / (1.0 + (-(x - location) / scale).exp()),
            Kind::Poisson(rate) => {
                if x < 0.0 {
                    0.0
                } else {
                    validated("Poisson", Poisson::new(*rate))?.cdf(x.floor() as u64)
                }
            }
            Kind::Triangular(low, mode, high) => {
                validated("Triangular", Triangular::new(*low, *high, *mode))?.cdf(x)
            }
            Kind::Samples(samples) => {
                samples.iter().filter(|sample| **sample <= x).count() as f64 / samples.len() as f64
            }
        })
    }

    /// Returns the density, or probability mass for a discrete family, at `x`.
    pub fn pdf(&self, x: f64) -> Result<f64, String> {
        if x.is_nan() {
            return Err("density input must not be NaN".into());
        }
        Ok(match &self.kind {
            Kind::Point(value) => f64::from(x == *value),
            Kind::Normal(mean, sigma) => validated("Normal", Normal::new(*mean, *sigma))?.pdf(x),
            Kind::Lognormal(mu, sigma) => {
                validated("Lognormal", LogNormal::new(*mu, *sigma))?.pdf(x)
            }
            Kind::Uniform(low, high) => validated("Uniform", Uniform::new(*low, *high))?.pdf(x),
            Kind::Beta(alpha, beta) => validated("Beta", Beta::new(*alpha, *beta))?.pdf(x),
            Kind::Bernoulli(p) => match discrete_mass(x) {
                Some(value) => validated("Bernoulli", Bernoulli::new(*p))?.pmf(value),
                None => 0.0,
            },
            Kind::Binomial(n, p) => match discrete_mass(x) {
                Some(value) => validated("Binomial", Binomial::new(*p, *n))?.pmf(value),
                None => 0.0,
            },
            Kind::Cauchy(location, scale) => {
                validated("Cauchy", Cauchy::new(*location, *scale))?.pdf(x)
            }
            Kind::Exponential(rate) => validated("Exponential", Exp::new(*rate))?.pdf(x),
            Kind::Gamma(shape, scale) => {
                validated("Gamma", Gamma::new(*shape, 1.0 / scale))?.pdf(x)
            }
            Kind::Logistic(location, scale) => {
                let e = (-(x - location) / scale).exp();
                e / (scale * (1.0 + e).powi(2))
            }
            Kind::Poisson(rate) => match discrete_mass(x) {
                Some(value) => validated("Poisson", Poisson::new(*rate))?.pmf(value),
                None => 0.0,
            },
            Kind::Triangular(low, mode, high) => {
                validated("Triangular", Triangular::new(*low, *high, *mode))?.pdf(x)
            }
            Kind::Samples(samples) => {
                samples.iter().filter(|sample| **sample == x).count() as f64 / samples.len() as f64
            }
        })
    }

    /// Returns the lower support bound, which may be negative infinity.
    pub fn minimum(&self) -> Result<f64, String> {
        Ok(match &self.kind {
            Kind::Point(value) => *value,
            Kind::Normal(..) | Kind::Cauchy(..) | Kind::Logistic(..) => f64::NEG_INFINITY,
            Kind::Lognormal(..)
            | Kind::Beta(..)
            | Kind::Bernoulli(_)
            | Kind::Binomial(..)
            | Kind::Exponential(_)
            | Kind::Gamma(..)
            | Kind::Poisson(_) => 0.0,
            Kind::Uniform(low, _) | Kind::Triangular(low, ..) => *low,
            Kind::Samples(samples) => samples
                .iter()
                .copied()
                .min_by(f64::total_cmp)
                .ok_or_else(|| "empirical distribution has no samples".to_owned())?,
        })
    }

    /// Returns the upper support bound, which may be positive infinity.
    pub fn maximum(&self) -> Result<f64, String> {
        Ok(match &self.kind {
            Kind::Point(value) => *value,
            Kind::Normal(..)
            | Kind::Lognormal(..)
            | Kind::Cauchy(..)
            | Kind::Exponential(_)
            | Kind::Gamma(..)
            | Kind::Logistic(..)
            | Kind::Poisson(_) => f64::INFINITY,
            Kind::Uniform(_, high) | Kind::Triangular(_, _, high) => *high,
            Kind::Beta(..) | Kind::Bernoulli(_) => 1.0,
            Kind::Binomial(n, _) => *n as f64,
            Kind::Samples(samples) => samples
                .iter()
                .copied()
                .max_by(f64::total_cmp)
                .ok_or_else(|| "empirical distribution has no samples".to_owned())?,
        })
    }

    /// Returns one modal value when the family has a defined mode.
    pub fn mode(&self) -> Result<f64, String> {
        match &self.kind {
            Kind::Point(value)
            | Kind::Normal(value, _)
            | Kind::Cauchy(value, _)
            | Kind::Logistic(value, _) => Ok(*value),
            Kind::Lognormal(mu, sigma) => Ok((mu - sigma * sigma).exp()),
            Kind::Uniform(low, high) => Ok((low + high) / 2.0),
            Kind::Beta(alpha, beta) if *alpha > 1.0 && *beta > 1.0 => {
                Ok((alpha - 1.0) / (alpha + beta - 2.0))
            }
            Kind::Beta(..) => Err("this Beta parameterization has no unique interior mode".into()),
            Kind::Bernoulli(p) => Ok(f64::from(*p >= 0.5)),
            Kind::Binomial(n, p) => Ok(((*n as f64 + 1.0) * p).floor().min(*n as f64)),
            Kind::Exponential(_) => Ok(0.0),
            Kind::Gamma(shape, scale) => Ok((shape - 1.0).max(0.0) * scale),
            Kind::Poisson(rate) => Ok(rate.floor()),
            Kind::Triangular(_, mode, _) => Ok(*mode),
            Kind::Samples(samples) => empirical_mode(samples),
        }
    }
}

fn empirical_quantile(samples: &[f64], p: f64) -> Result<f64, String> {
    if samples.is_empty() {
        return Err("empirical distribution has no samples".into());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let position = p * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    let lower = sorted
        .get(lower)
        .copied()
        .ok_or_else(|| "empirical quantile index is out of bounds".to_owned())?;
    let upper = sorted
        .get(upper)
        .copied()
        .ok_or_else(|| "empirical quantile index is out of bounds".to_owned())?;
    Ok(lower + (upper - lower) * position.fract())
}

fn empirical_mode(samples: &[f64]) -> Result<f64, String> {
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let first = sorted
        .first()
        .copied()
        .ok_or_else(|| "empirical distribution has no samples".to_owned())?;
    let mut best = (first, 1usize);
    let mut current = best;
    for value in sorted.into_iter().skip(1) {
        if value == current.0 {
            current.1 += 1;
        } else {
            current = (value, 1);
        }
        if current.1 > best.1 {
            best = current;
        }
    }
    Ok(best.0)
}

fn validated<T, E: fmt::Display>(family: &str, result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("invalid {family} distribution: {error}"))
}

fn discrete_mass(value: f64) -> Option<u64> {
    (value >= 0.0 && value.is_finite() && value.fract() == 0.0).then_some(value as u64)
}
