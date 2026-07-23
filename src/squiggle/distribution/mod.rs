//! Validated probability distributions used by the Squiggle runtime.
//!
//! The symbolic families follow Squiggle's parameterizations: Normal uses
//! $(\mu,\sigma)$, Lognormal uses the log-space $(\mu,\sigma)$, Gamma uses
//! shape/scale, Exponential uses rate, and Triangular uses minimum/mode/maximum.
//! Every constructor validates support and finite parameters. Analytical CDF,
//! density, quantile, mean, and variance calculations delegate to `statrs` where
//! available; logistic and empirical formulas are implemented directly.
//!
//! Sampling applies inverse transform sampling to one deterministic ChaCha20
//! uniform draw. Empirical distributions use linear quantiles. Composed runtime
//! distributions are finite Monte Carlo approximations, so their tail accuracy is
//! limited by the configured sample count and must not be presented as exact.
//!
//! References: NIST/SEMATECH e-Handbook of Statistical Methods, sections 1.3.6
//! and 1.3.6.6; Luc Devroye, *Non-Uniform Random Variate Generation* (1986).

mod sample;
mod stats;

/// A validated symbolic or empirical scalar probability distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct Distribution(pub(super) Kind);

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    Point(f64),
    Normal(f64, f64),
    Lognormal(f64, f64),
    Uniform(f64, f64),
    Beta(f64, f64),
    Bernoulli(f64),
    Binomial(u64, f64),
    Cauchy(f64, f64),
    Exponential(f64),
    Gamma(f64, f64),
    Logistic(f64, f64),
    Poisson(f64),
    Triangular(f64, f64, f64),
    Samples(Vec<f64>),
}

impl Distribution {
    /// Creates a deterministic point mass.
    pub fn point(value: f64) -> Result<Self, String> {
        finite(value, "point value")?;
        Ok(Self(Kind::Point(value)))
    }

    /// Creates a Normal distribution from mean and positive standard deviation.
    ///
    /// ```
    /// let normal = optimist::squiggle::Distribution::normal(10.0, 2.0)?;
    /// assert_eq!(normal.mean()?, 10.0);
    /// # Ok::<(), String>(())
    /// ```
    pub fn normal(mean: f64, stdev: f64) -> Result<Self, String> {
        finite(mean, "mean")?;
        positive(stdev, "standard deviation")?;
        Ok(Self(Kind::Normal(mean, stdev)))
    }

    /// Creates a Lognormal distribution from log-space location and scale.
    pub fn lognormal(mu: f64, sigma: f64) -> Result<Self, String> {
        finite(mu, "log-space location")?;
        positive(sigma, "log-space scale")?;
        Ok(Self(Kind::Lognormal(mu, sigma)))
    }

    /// Creates a continuous Uniform distribution on `[minimum, maximum]`.
    pub fn uniform(minimum: f64, maximum: f64) -> Result<Self, String> {
        ordered(minimum, maximum)?;
        Ok(Self(Kind::Uniform(minimum, maximum)))
    }

    /// Creates a Beta distribution from positive shape parameters.
    pub fn beta(alpha: f64, beta: f64) -> Result<Self, String> {
        positive(alpha, "alpha")?;
        positive(beta, "beta")?;
        Ok(Self(Kind::Beta(alpha, beta)))
    }

    /// Creates a Bernoulli distribution with success probability `p`.
    pub fn bernoulli(p: f64) -> Result<Self, String> {
        probability(p)?;
        Ok(Self(Kind::Bernoulli(p)))
    }

    /// Creates a Binomial distribution for `trials` independent Bernoulli events.
    pub fn binomial(trials: u64, p: f64) -> Result<Self, String> {
        probability(p)?;
        Ok(Self(Kind::Binomial(trials, p)))
    }

    /// Creates a Cauchy distribution from location and positive scale.
    pub fn cauchy(location: f64, scale: f64) -> Result<Self, String> {
        finite(location, "location")?;
        positive(scale, "scale")?;
        Ok(Self(Kind::Cauchy(location, scale)))
    }

    /// Creates an Exponential distribution with positive event rate.
    pub fn exponential(rate: f64) -> Result<Self, String> {
        positive(rate, "rate")?;
        Ok(Self(Kind::Exponential(rate)))
    }

    /// Creates a Gamma distribution using Squiggle's positive shape and scale.
    pub fn gamma(shape: f64, scale: f64) -> Result<Self, String> {
        positive(shape, "shape")?;
        positive(scale, "scale")?;
        Ok(Self(Kind::Gamma(shape, scale)))
    }

    /// Creates a Logistic distribution from location and positive scale.
    pub fn logistic(location: f64, scale: f64) -> Result<Self, String> {
        finite(location, "location")?;
        positive(scale, "scale")?;
        Ok(Self(Kind::Logistic(location, scale)))
    }

    /// Creates a Poisson count distribution with positive event rate.
    pub fn poisson(rate: f64) -> Result<Self, String> {
        positive(rate, "rate")?;
        Ok(Self(Kind::Poisson(rate)))
    }

    /// Creates a Triangular distribution from minimum, mode, and maximum.
    pub fn triangular(minimum: f64, mode: f64, maximum: f64) -> Result<Self, String> {
        ordered(minimum, maximum)?;
        if !(minimum..=maximum).contains(&mode) {
            return Err("mode must lie between minimum and maximum".into());
        }
        Ok(Self(Kind::Triangular(minimum, mode, maximum)))
    }

    /// Creates an empirical distribution from finite scalar draws.
    pub fn from_samples(samples: Vec<f64>) -> Result<Self, String> {
        if samples.is_empty() {
            return Err("an empirical distribution requires at least one sample".into());
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err("empirical samples must all be finite".into());
        }
        Ok(Self(Kind::Samples(samples)))
    }

    /// Returns the canonical Squiggle family name.
    pub fn family(&self) -> &'static str {
        match self.0 {
            Kind::Point(_) => "PointMass",
            Kind::Normal(..) => "Normal",
            Kind::Lognormal(..) => "Lognormal",
            Kind::Uniform(..) => "Uniform",
            Kind::Beta(..) => "Beta",
            Kind::Bernoulli(_) => "Bernoulli",
            Kind::Binomial(..) => "Binomial",
            Kind::Cauchy(..) => "Cauchy",
            Kind::Exponential(_) => "Exponential",
            Kind::Gamma(..) => "Gamma",
            Kind::Logistic(..) => "Logistic",
            Kind::Poisson(_) => "Poisson",
            Kind::Triangular(..) => "Triangular",
            Kind::Samples(_) => "SampleSet",
        }
    }

    /// Borrows the stored draws when this is an empirical sample set.
    pub fn samples(&self) -> Option<&[f64]> {
        if let Kind::Samples(samples) = &self.0 {
            Some(samples)
        } else {
            None
        }
    }

    /// Returns the value when this distribution is a symbolic point mass.
    pub fn point_value(&self) -> Option<f64> {
        match self.0 {
            Kind::Point(value) => Some(value),
            _ => None,
        }
    }

    /// Returns `(mean, standard_deviation)` for a symbolic Normal distribution.
    pub fn normal_parameters(&self) -> Option<(f64, f64)> {
        match self.0 {
            Kind::Normal(mean, standard_deviation) => Some((mean, standard_deviation)),
            _ => None,
        }
    }

    /// Returns `(location, scale)` for a symbolic Lognormal distribution.
    pub fn lognormal_parameters(&self) -> Option<(f64, f64)> {
        match self.0 {
            Kind::Lognormal(location, scale) => Some((location, scale)),
            _ => None,
        }
    }

    /// Returns `(alpha, beta)` for a symbolic Beta distribution.
    pub fn beta_parameters(&self) -> Option<(f64, f64)> {
        match self.0 {
            Kind::Beta(alpha, beta) => Some((alpha, beta)),
            _ => None,
        }
    }
}

fn finite(value: f64, name: &str) -> Result<(), String> {
    value
        .is_finite()
        .then_some(())
        .ok_or_else(|| format!("{name} must be finite"))
}

fn positive(value: f64, name: &str) -> Result<(), String> {
    finite(value, name)?;
    (value > 0.0)
        .then_some(())
        .ok_or_else(|| format!("{name} must be greater than zero"))
}

fn probability(value: f64) -> Result<(), String> {
    finite(value, "probability")?;
    (0.0..=1.0)
        .contains(&value)
        .then_some(())
        .ok_or_else(|| "probability must be between zero and one".into())
}

fn ordered(minimum: f64, maximum: f64) -> Result<(), String> {
    finite(minimum, "minimum")?;
    finite(maximum, "maximum")?;
    (minimum < maximum)
        .then_some(())
        .ok_or_else(|| "minimum must be less than maximum".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytical_moments_match_parameterizations() -> Result<(), String> {
        assert_eq!(Distribution::normal(4.0, 3.0)?.variance()?, 9.0);
        assert_eq!(Distribution::gamma(4.0, 3.0)?.mean()?, 12.0);
        assert!((Distribution::beta(2.0, 3.0)?.mean()? - 0.4).abs() < 1e-15);
        Ok(())
    }

    #[test]
    fn continuous_quantiles_invert_cdfs() -> Result<(), String> {
        for distribution in [
            Distribution::normal(2.0, 4.0)?,
            Distribution::lognormal(1.0, 0.4)?,
            Distribution::gamma(3.0, 2.0)?,
            Distribution::logistic(-1.0, 0.5)?,
        ] {
            let value = distribution.quantile(0.83)?;
            assert!(
                (distribution.cdf(value)? - 0.83).abs() < 2e-5,
                "{}",
                distribution.family()
            );
        }
        Ok(())
    }

    #[test]
    fn seeded_samples_replay_exactly() -> Result<(), String> {
        let distribution = Distribution::normal(0.0, 1.0)?;
        assert_eq!(
            distribution.sample_seeded(42),
            distribution.sample_seeded(42)
        );
        assert_ne!(
            distribution.sample_seeded(42),
            distribution.sample_seeded(43)
        );
        Ok(())
    }

    #[test]
    fn malformed_queries_return_errors() -> Result<(), String> {
        let distribution = Distribution::normal(0.0, 1.0)?;
        assert!(distribution.quantile(f64::NAN).is_err());
        assert!(distribution.cdf(f64::NAN).is_err());
        assert!(distribution.pdf(f64::NAN).is_err());
        assert!(Distribution::normal(0.0, 0.0).is_err());
        assert!(Distribution::from_samples(Vec::new()).is_err());
        Ok(())
    }
}
