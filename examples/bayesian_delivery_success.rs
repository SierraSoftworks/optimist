use optimist::domain::{BetaBinomialLikelihood, Distribution};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prior = Distribution::beta(2.0, 2.0)?;
    let observed_rollouts = BetaBinomialLikelihood::new(17, 20)?;
    let posterior = prior.update_beta_binomial(observed_rollouts)?;

    println!(
        "Prior success probability: mean={:.3}, variance={:.5}",
        prior.mean(),
        prior.variance()
    );
    println!(
        "Posterior after 17/20 successful rollouts: mean={:.3}, variance={:.5}",
        posterior.mean(),
        posterior.variance()
    );

    assert!(posterior.mean() > prior.mean());
    assert!(posterior.variance() < prior.variance());
    Ok(())
}
