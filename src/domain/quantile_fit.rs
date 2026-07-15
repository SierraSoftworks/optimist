use statrs::distribution::{ContinuousCDF, Normal};

use super::{
    Distribution, FitDiagnostics, FittedDistribution, QuantileElicitation, QuantileFitError,
};

#[derive(Clone, Copy)]
enum Transform {
    Identity,
    Log,
}

pub(super) fn normal(
    elicitation: QuantileElicitation,
) -> Result<FittedDistribution, QuantileFitError> {
    fit(elicitation, Transform::Identity)
}

pub(super) fn log_normal(
    elicitation: QuantileElicitation,
) -> Result<FittedDistribution, QuantileFitError> {
    fit(elicitation, Transform::Log)
}

fn fit(
    elicitation: QuantileElicitation,
    transform: Transform,
) -> Result<FittedDistribution, QuantileFitError> {
    validate(&elicitation, transform)?;
    let probabilities = [
        elicitation.lower_probability,
        0.5,
        elicitation.upper_probability,
    ];
    let entered = [elicitation.lower, elicitation.median, elicitation.upper];
    let standard_normal = Normal::new(0.0, 1.0).expect("standard Normal parameters are valid");
    let z = probabilities.map(|probability| standard_normal.inverse_cdf(probability));
    let values = entered.map(|value| match transform {
        Transform::Identity => value,
        Transform::Log => value.ln(),
    });
    let (location, scale) = least_squares(&z, &values)?;
    let fitted = z.map(|score| match transform {
        Transform::Identity => location + scale * score,
        Transform::Log => (location + scale * score).exp(),
    });
    let errors = [
        fitted[0] - entered[0],
        fitted[1] - entered[1],
        fitted[2] - entered[2],
    ];
    let distribution = match transform {
        Transform::Identity => Distribution::normal(location, scale)?,
        Transform::Log => Distribution::log_normal(location, scale)?,
    };
    Ok(FittedDistribution {
        elicitation,
        distribution,
        diagnostics: FitDiagnostics {
            root_mean_squared_error: (errors.iter().map(|error| error * error).sum::<f64>() / 3.0)
                .sqrt(),
            maximum_absolute_error: errors.iter().map(|error| error.abs()).fold(0.0, f64::max),
            fitted_lower: fitted[0],
            fitted_median: fitted[1],
            fitted_upper: fitted[2],
        },
    })
}

fn validate(
    elicitation: &QuantileElicitation,
    transform: Transform,
) -> Result<(), QuantileFitError> {
    let values = [
        elicitation.lower_probability,
        elicitation.lower,
        elicitation.median,
        elicitation.upper_probability,
        elicitation.upper,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return Err(QuantileFitError::NonFinite);
    }
    if !(0.0 < elicitation.lower_probability
        && elicitation.lower_probability < 0.5
        && 0.5 < elicitation.upper_probability
        && elicitation.upper_probability < 1.0)
    {
        return Err(QuantileFitError::InvalidProbabilities);
    }
    if !(elicitation.lower <= elicitation.median && elicitation.median <= elicitation.upper) {
        return Err(QuantileFitError::InvalidOrder);
    }
    if matches!(transform, Transform::Log) && elicitation.lower <= 0.0 {
        return Err(QuantileFitError::NonPositiveLogNormalValue);
    }
    Ok(())
}

fn least_squares(z: &[f64; 3], values: &[f64; 3]) -> Result<(f64, f64), QuantileFitError> {
    let z_mean = z.iter().sum::<f64>() / 3.0;
    let value_mean = values.iter().sum::<f64>() / 3.0;
    let denominator = z.iter().map(|score| (score - z_mean).powi(2)).sum::<f64>();
    let scale = z
        .iter()
        .zip(values)
        .map(|(score, value)| (score - z_mean) * (value - value_mean))
        .sum::<f64>()
        / denominator;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(QuantileFitError::InvalidScale);
    }
    Ok((value_mean - scale * z_mean, scale))
}
