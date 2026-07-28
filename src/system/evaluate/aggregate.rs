//! Reducing several arrivals of one signal to the figure a component reads.

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        compile::Plan,
        signal::Aggregation,
        values::{draws, from_draws},
    },
};

use super::config::EvaluationConfig;

/// Every signal the catalogue knows about, at rest.
///
/// Success rests at one rather than zero. A component with nothing attached is
/// not failing, and starting the relaxation at zero would make every unattached
/// dependency look like a total outage.
pub(super) fn blank(plan: &Plan) -> std::collections::BTreeMap<String, Value> {
    plan.signals
        .iter()
        .map(|(signal, declaration)| {
            (
                signal.clone(),
                Value::Number(identity(declaration.aggregate)),
            )
        })
        .collect()
}

/// What a signal reads when nothing arrives carrying it.
///
/// The identity of the aggregation, so that combining nothing leaves the reader
/// unaffected. Zero is right for a rate, which nobody is offering, and wrong for
/// a success rate: a component with no dependencies depends on nothing that
/// could fail, and reading zero there would report every leaf of a design as a
/// total outage and propagate that back to the caller.
fn identity(aggregation: Aggregation) -> f64 {
    match aggregation {
        Aggregation::Product => 1.0,
        // Nothing attached imposes no ceiling, so the limit is unbounded rather
        // than nought; reading zero would report an unattached port as unable to
        // carry anything at all.
        Aggregation::Min => f64::INFINITY,
        Aggregation::Sum | Aggregation::Max | Aggregation::Mean => 0.0,
    }
}

/// Reduces several arrivals of one signal to the figure a component reads.
pub(super) fn combine(
    values: &[Value],
    aggregation: Aggregation,
    divisor: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> Value {
    if values.is_empty() {
        return Value::Number(identity(aggregation));
    }
    let count = values
        .iter()
        .filter_map(|value| match value {
            Value::Distribution(distribution) => distribution.samples().map(<[f64]>::len),
            _ => None,
        })
        .min()
        .unwrap_or(config.sample_count.max(1));
    let columns = values
        .iter()
        .filter_map(|value| draws(value, count, rng))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Value::Number(identity(aggregation));
    }
    let scale = if divisor > 0.0 { divisor } else { 1.0 };
    let combined = (0..count)
        .map(|index| {
            let mut row = columns.iter().map(|column| column[index]);
            let first = row.next().unwrap_or(0.0);
            let value = match aggregation {
                Aggregation::Sum => first + row.sum::<f64>(),
                Aggregation::Max => row.fold(first, f64::max),
                Aggregation::Product => row.fold(first, |total, next| total * next),
                Aggregation::Min => row.fold(first, f64::min),
                Aggregation::Mean => {
                    (first + row.sum::<f64>())
                        / f64::from(u32::try_from(columns.len()).unwrap_or(1))
                }
            };
            value / scale
        })
        .collect::<Vec<_>>();
    from_draws(combined).unwrap_or(Value::Number(0.0))
}
