//! Monte Carlo forecast scoring.
//!
//! Distribution scoring estimates $D_{KL}(A\Vert E)=E_A[\ln p_A(X)-\ln
//! p_E(X)]$. Scalar scoring is $-\ln p_E(y)$. When a prior is supplied, its
//! corresponding score is subtracted. Estimates use the runtime's seeded sample
//! count, report no quadrature error bound, and clamp zero densities to the
//! smallest positive `f64`; they are unsuitable for singular mixed measures.

use crate::squiggle::{Diagnostic, Distribution, Value, ast::Span};

use super::Runtime;

builtins! {
    context(runtime, span);
    "Dist.klDivergence"(estimate: Distribution, answer: Distribution) => {
        Ok(Value::Number(kl(runtime, answer, estimate, span)?))
    },
    "Dist.logScore"(fields: Dictionary) => log_score(runtime, fields, span),
}

fn log_score(
    runtime: &mut Runtime,
    fields: &std::collections::BTreeMap<String, Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let estimate_value = fields
        .get("estimate")
        .ok_or_else(|| Diagnostic::runtime("logScore requires estimate", span))?;
    let answer = fields
        .get("answer")
        .ok_or_else(|| Diagnostic::runtime("logScore requires answer", span))?;
    let estimate = distribution(estimate_value, span)?;
    let score = match answer {
        Value::Number(answer) => -density(estimate, *answer, span)?.ln(),
        Value::Distribution(answer) => kl(runtime, answer, estimate, span)?,
        value => return Err(expected("Number or Distribution answer", value, span)),
    };
    let prior_score = match (fields.get("prior"), answer) {
        (Some(prior), Value::Number(answer)) => {
            -density(distribution(prior, span)?, *answer, span)?.ln()
        }
        (Some(prior), Value::Distribution(answer)) => {
            kl(runtime, answer, distribution(prior, span)?, span)?
        }
        _ => 0.0,
    };
    Ok(Value::Number(score - prior_score))
}

fn kl(
    runtime: &mut Runtime,
    answer: &Distribution,
    estimate: &Distribution,
    span: Span,
) -> Result<f64, Diagnostic> {
    let total = (0..runtime.config.sample_count)
        .map(|_| {
            let value = answer
                .sample(&mut runtime.rng)
                .map_err(|error| Diagnostic::runtime(error, span))?;
            Ok((density(answer, value, span)? / density(estimate, value, span)?).ln())
        })
        .sum::<Result<f64, Diagnostic>>()?;
    Ok(total / runtime.config.sample_count as f64)
}

fn density(distribution: &Distribution, value: f64, span: Span) -> Result<f64, Diagnostic> {
    distribution
        .pdf(value)
        .map(|density| density.max(f64::MIN_POSITIVE))
        .map_err(|error| Diagnostic::runtime(error, span))
}

fn distribution(value: &Value, span: Span) -> Result<&Distribution, Diagnostic> {
    value
        .as_distribution()
        .ok_or_else(|| expected("Distribution", value, span))
}

fn expected(expected: &str, value: &Value, span: Span) -> Diagnostic {
    Diagnostic::runtime(
        format!("expected {expected}, received {}", value.type_name()),
        span,
    )
}
