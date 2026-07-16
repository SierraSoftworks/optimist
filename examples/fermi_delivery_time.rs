use optimist::domain::{Distribution, Formula, FormulaSet, MonteCarloConfig, ProjectId, Unit};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let deployments = Formula::Literal {
        distribution: Distribution::scaled_beta(5.0, 3.0, 8.0, 30.0)?,
        unit: Unit::base("deployments")?,
    };
    let minutes_per_deployment = Formula::Literal {
        distribution: Distribution::log_normal(3.4, 0.35)?,
        unit: Unit::from_exponents([("minutes", 1), ("deployments", -1)])?,
    };
    let monthly_delivery_time = Formula::Product {
        factors: vec![deployments, minutes_per_deployment],
    };
    let formulas = FormulaSet::default();
    let project = ProjectId::new("delivery")?;

    let compiled = formulas.validate(&project, &monthly_delivery_time)?;
    assert_eq!(compiled.unit.exponent("minutes"), 1);
    assert_eq!(compiled.unit.exponent("deployments"), 0);

    let report = formulas.sample_joint(
        &project,
        &[monthly_delivery_time],
        MonteCarloConfig::new(42, 10_000, 100_000, 0.5, 0.002)?,
    )?;
    let estimate = &report.estimates[0];

    println!(
        "Expected monthly delivery time: {:.1} minutes",
        estimate.mean.expect("valid draws produce a mean")
    );
    println!(
        "Model variance: {:.1}; Monte Carlo mean SE: {:.3}",
        estimate.variance.expect("valid draws produce variance"),
        estimate
            .mean_standard_error
            .expect("valid draws produce a mean standard error")
    );
    println!(
        "Samples: {} valid / {} attempted ({:?})",
        report.diagnostics.valid_samples,
        report.diagnostics.attempted_samples,
        report.diagnostics.status
    );
    Ok(())
}
