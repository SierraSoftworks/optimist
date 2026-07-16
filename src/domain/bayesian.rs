use super::estimate::DistributionKind;
use super::{BayesianUpdateError, BetaBinomialLikelihood, Distribution, NormalNormalLikelihood};

impl Distribution {
    /// Updates a Beta prior with a Binomial likelihood.
    ///
    /// For prior $p\sim\operatorname{Beta}(\alpha,\beta)$ and $s$ successes in
    /// $n$ conditionally independent trials, the posterior is
    /// $\operatorname{Beta}(\alpha+s,\beta+n-s)$. Sequential updates are therefore
    /// order invariant up to floating-point addition. This API accepts only an
    /// unscaled Beta because affine bounds do not describe a Bernoulli probability.
    pub fn update_beta_binomial(
        &self,
        likelihood: BetaBinomialLikelihood,
    ) -> Result<Self, BayesianUpdateError> {
        let DistributionKind::Beta { alpha, beta } = self.0 else {
            return Err(BayesianUpdateError::ExpectedBetaPrior);
        };
        let posterior_alpha = alpha + likelihood.successes() as f64;
        let posterior_beta = beta + (likelihood.trials() - likelihood.successes()) as f64;
        if !posterior_alpha.is_finite() || !posterior_beta.is_finite() {
            return Err(BayesianUpdateError::NonFinitePosterior);
        }
        Distribution::beta(posterior_alpha, posterior_beta)
            .map_err(|_| BayesianUpdateError::NonFinitePosterior)
    }

    /// Updates a Normal prior for an unknown mean using known-variance Normal data.
    ///
    /// With prior $\mu\sim N(m_0,v_0)$ and likelihood sample mean $\bar{x}$ from
    /// $n$ independent $N(\mu,\sigma^2)$ observations, posterior precision is
    /// $1/v_n=1/v_0+n/\sigma^2$ and
    /// $m_n=v_n(m_0/v_0+n\bar{x}/\sigma^2)$. The update does not infer
    /// $\sigma^2$ and is not suitable when observation variance is unknown.
    pub fn update_normal_normal(
        &self,
        likelihood: NormalNormalLikelihood,
    ) -> Result<Self, BayesianUpdateError> {
        let DistributionKind::Normal {
            mean,
            standard_deviation,
        } = self.0
        else {
            return Err(BayesianUpdateError::ExpectedNormalPrior);
        };
        let prior_variance = standard_deviation.powi(2);
        let prior_precision = prior_variance.recip();
        let data_precision = likelihood.sample_count() as f64 / likelihood.known_variance();
        let posterior_variance = (prior_precision + data_precision).recip();
        let posterior_mean = posterior_variance
            * (mean * prior_precision + likelihood.sample_mean() * data_precision);
        if posterior_variance <= 0.0
            || !posterior_variance.is_finite()
            || !posterior_mean.is_finite()
        {
            return Err(BayesianUpdateError::NonFinitePosterior);
        }
        Distribution::normal(posterior_mean, posterior_variance.sqrt())
            .map_err(|_| BayesianUpdateError::NonFinitePosterior)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_updates_are_order_equivalent_and_contract() {
        let prior = Distribution::beta(2.0, 2.0).unwrap();
        let first = BetaBinomialLikelihood::new(7, 10).unwrap();
        let second = BetaBinomialLikelihood::new(2, 5).unwrap();
        let sequential = prior
            .update_beta_binomial(first)
            .unwrap()
            .update_beta_binomial(second)
            .unwrap();
        let reversed = prior
            .update_beta_binomial(second)
            .unwrap()
            .update_beta_binomial(first)
            .unwrap();
        assert_eq!(sequential, reversed);
        assert!(sequential.variance() < prior.variance());
        assert!((sequential.mean() - 11.0 / 19.0).abs() < 1e-12);
    }

    #[test]
    fn normal_update_matches_precision_equations_and_contracts() {
        let prior = Distribution::normal(10.0, 2.0).unwrap();
        let likelihood = NormalNormalLikelihood::new(12.0, 9.0, 9).unwrap();
        let posterior = prior.update_normal_normal(likelihood).unwrap();
        assert!((posterior.mean() - 11.6).abs() < 1e-12);
        assert!((posterior.variance() - 0.8).abs() < 1e-12);
        assert!(posterior.variance() < prior.variance());
    }

    #[test]
    fn normal_updates_match_combined_sufficient_statistics() {
        let prior = Distribution::normal(-1.0, 3.0).unwrap();
        let first = NormalNormalLikelihood::new(2.0, 4.0, 5).unwrap();
        let second = NormalNormalLikelihood::new(5.0, 4.0, 15).unwrap();
        let sequential = prior
            .update_normal_normal(first)
            .unwrap()
            .update_normal_normal(second)
            .unwrap();
        let reversed = prior
            .update_normal_normal(second)
            .unwrap()
            .update_normal_normal(first)
            .unwrap();
        let combined = prior
            .update_normal_normal(NormalNormalLikelihood::new(4.25, 4.0, 20).unwrap())
            .unwrap();
        assert!((sequential.mean() - reversed.mean()).abs() < 1e-12);
        assert!((sequential.mean() - combined.mean()).abs() < 1e-12);
        assert!((sequential.variance() - combined.variance()).abs() < 1e-12);
    }

    #[test]
    fn likelihoods_and_prior_families_are_validated() {
        assert_eq!(
            BetaBinomialLikelihood::new(2, 1),
            Err(BayesianUpdateError::InvalidBinomialCounts)
        );
        assert_eq!(
            NormalNormalLikelihood::new(0.0, 0.0, 1),
            Err(BayesianUpdateError::InvalidKnownVariance)
        );
        assert_eq!(
            Distribution::point(0.5)
                .unwrap()
                .update_beta_binomial(BetaBinomialLikelihood::new(1, 1).unwrap()),
            Err(BayesianUpdateError::ExpectedBetaPrior)
        );
    }
}
