use thiserror::Error;

use super::Distribution;
use super::estimate::DistributionKind;

/// Failures in analytical distribution composition.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PropagationError {
    /// The requested operation does not preserve the supplied distribution families.
    #[error("analytical propagation requires {0} operands")]
    FamilyMismatch(&'static str),
    /// Covariance was NaN, infinite, or violated $|cov(X,Y)| <= \sigma_X\sigma_Y$.
    #[error("covariance is not finite or violates the Cauchy-Schwarz bound")]
    InvalidCovariance,
    /// The resulting variance was negative by more than floating-point tolerance.
    #[error("covariance produces a negative resulting variance")]
    NegativeVariance,
    /// The exact result exceeded finite floating-point range.
    #[error("propagated parameters are outside finite floating-point range")]
    NonFiniteResult,
}

impl Distribution {
    /// Returns the exact distribution of the sum of two jointly Normal variables.
    ///
    /// For means $\mu_X,\mu_Y$, variances $v_X,v_Y$, and supplied covariance
    /// $c=\operatorname{Cov}(X,Y)$, the result is Normal with mean $\mu_X+\mu_Y$
    /// and variance $v_X+v_Y+2c$. The covariance must satisfy Cauchy-Schwarz.
    /// A variance within 64 machine epsilons of zero becomes a point mass; a more
    /// negative value is rejected. The caller, not this function, is responsible
    /// for establishing a valid joint Normal dependence model. See Tong,
    /// *The Multivariate Normal Distribution* (1990), chapter 2.
    pub fn normal_sum(&self, other: &Self, covariance: f64) -> Result<Self, PropagationError> {
        let DistributionKind::Normal {
            mean: left_mean,
            standard_deviation: left_scale,
        } = self.0
        else {
            return Err(PropagationError::FamilyMismatch("Normal"));
        };
        let DistributionKind::Normal {
            mean: right_mean,
            standard_deviation: right_scale,
        } = other.0
        else {
            return Err(PropagationError::FamilyMismatch("Normal"));
        };
        validate_covariance(covariance, left_scale, right_scale)?;
        from_location_variance(
            left_mean + right_mean,
            left_scale.powi(2) + right_scale.powi(2) + 2.0 * covariance,
            Distribution::normal,
        )
    }

    /// Returns the exact product of two jointly log-Normal variables.
    ///
    /// If $(\log X,\log Y)$ is jointly Normal with covariance $c$, then
    /// $\log(XY)$ has location $\mu_X+\mu_Y$ and variance
    /// $\sigma_X^2+\sigma_Y^2+2c$. `covariance` is therefore in log space.
    /// Passing zero asserts log-space independence (or merely zero covariance
    /// under joint Normality). Arbitrary non-Gaussian dependence is unsupported.
    /// See Aitchison and Brown, *The Lognormal Distribution* (1957), chapter 2.
    pub fn log_normal_product(
        &self,
        other: &Self,
        log_covariance: f64,
    ) -> Result<Self, PropagationError> {
        self.log_normal_compose(other, log_covariance, false)
    }

    /// Returns the exact ratio of two jointly log-Normal variables.
    ///
    /// If $(\log X,\log Y)$ is jointly Normal with covariance $c$, then
    /// $\log(X/Y)$ has location $\mu_X-\mu_Y$ and variance
    /// $\sigma_X^2+\sigma_Y^2-2c$. `covariance` is in log space; zero states
    /// log-space independence or zero correlation. The denominator is positive
    /// almost surely by the LogNormal model, unlike generic formula ratios.
    pub fn log_normal_ratio(
        &self,
        other: &Self,
        log_covariance: f64,
    ) -> Result<Self, PropagationError> {
        self.log_normal_compose(other, log_covariance, true)
    }

    fn log_normal_compose(
        &self,
        other: &Self,
        covariance: f64,
        ratio: bool,
    ) -> Result<Self, PropagationError> {
        let DistributionKind::LogNormal { location, scale } = self.0 else {
            return Err(PropagationError::FamilyMismatch("LogNormal"));
        };
        let DistributionKind::LogNormal {
            location: other_location,
            scale: other_scale,
        } = other.0
        else {
            return Err(PropagationError::FamilyMismatch("LogNormal"));
        };
        validate_covariance(covariance, scale, other_scale)?;
        let sign = if ratio { -1.0 } else { 1.0 };
        let result_location = location + sign * other_location;
        let result_variance = scale.powi(2) + other_scale.powi(2) + sign * 2.0 * covariance;
        let tolerance = 64.0 * f64::EPSILON * result_variance.abs().max(1.0);
        if result_variance >= -tolerance && result_variance <= 0.0 {
            return Distribution::point(result_location.exp())
                .map_err(|_| PropagationError::NonFiniteResult);
        }
        from_location_variance(result_location, result_variance, Distribution::log_normal)
    }
}

fn validate_covariance(
    covariance: f64,
    left_scale: f64,
    right_scale: f64,
) -> Result<(), PropagationError> {
    let bound = left_scale * right_scale;
    let tolerance = 64.0 * f64::EPSILON * bound.max(1.0);
    if !covariance.is_finite() || covariance.abs() > bound + tolerance {
        return Err(PropagationError::InvalidCovariance);
    }
    Ok(())
}

fn from_location_variance(
    location: f64,
    variance: f64,
    constructor: fn(f64, f64) -> Result<Distribution, super::DistributionError>,
) -> Result<Distribution, PropagationError> {
    let tolerance = 64.0 * f64::EPSILON * variance.abs().max(1.0);
    if variance < -tolerance {
        return Err(PropagationError::NegativeVariance);
    }
    if !location.is_finite() || !variance.is_finite() {
        return Err(PropagationError::NonFiniteResult);
    }
    if variance <= 0.0 {
        return Distribution::point(location).map_err(|_| PropagationError::NonFiniteResult);
    }
    constructor(location, variance.sqrt()).map_err(|_| PropagationError::NonFiniteResult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_sum_includes_twice_covariance() {
        let left = Distribution::normal(2.0, 3.0).unwrap();
        let right = Distribution::normal(5.0, 4.0).unwrap();
        let sum = left.normal_sum(&right, -6.0).unwrap();
        assert_eq!(sum.mean(), 7.0);
        assert!((sum.variance() - 13.0).abs() < 1e-12);
        assert_eq!(
            left.normal_sum(&right, -13.0),
            Err(PropagationError::InvalidCovariance)
        );
    }

    #[test]
    fn log_normal_product_and_ratio_compose_in_log_space() {
        let left = Distribution::log_normal(2.0, 0.5).unwrap();
        let right = Distribution::log_normal(1.0, 0.25).unwrap();
        let product = left.log_normal_product(&right, 0.05).unwrap();
        let ratio = left.log_normal_ratio(&right, 0.05).unwrap();
        assert!((product.mean().ln() - (3.0 + 0.4125 / 2.0)).abs() < 1e-12);
        assert!((ratio.mean().ln() - (1.0 + 0.2125 / 2.0)).abs() < 1e-12);
    }

    #[test]
    fn perfectly_correlated_log_ratio_becomes_a_point() {
        let left = Distribution::log_normal(2.0, 0.5).unwrap();
        let right = Distribution::log_normal(1.0, 0.5).unwrap();
        let ratio = left.log_normal_ratio(&right, 0.25).unwrap();
        assert_eq!(ratio.variance(), 0.0);
        assert!((ratio.mean() - 1.0_f64.exp()).abs() < 1e-12);
    }
}
