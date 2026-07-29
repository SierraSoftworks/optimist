//! Reducing several arrivals of one signal to the figure a component reads.

use rand_chacha::ChaCha20Rng;

use crate::{
    squiggle::Value,
    system::{
        compile::Plan,
        signal::{Aggregation, Signal},
        values::{Varying, aligned, all_uniform, per_draw},
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
        .map(|(signal, declaration)| (signal.clone(), Value::Number(declaration.rest())))
        .collect()
}

/// Reduces several arrivals of one signal to the figure a component reads.
pub(super) fn combine(
    values: &[Value],
    declaration: &Signal,
    divisor: f64,
    config: EvaluationConfig,
    rng: &mut ChaCha20Rng,
) -> Value {
    let aggregation = declaration.aggregate;
    if values.is_empty() {
        return Value::Number(declaration.rest());
    }
    let scale = if divisor > 0.0 { divisor } else { 1.0 };
    // One arrival reduces to itself under every aggregation, so an unshared
    // signal travelling a single relationship is carried through untouched.
    if let ([only], 1.0) = (values, scale) {
        return only.clone();
    }
    let ensemble = values
        .iter()
        .filter_map(|value| match value {
            Value::Distribution(distribution) => distribution.samples().map(<[f64]>::len),
            _ => None,
        })
        .min()
        .map_or_else(|| config.ensemble(), |authored| {
            config.ensemble().resized(authored)
        });
    let count = ensemble.len();
    let columns = values
        .iter()
        .filter_map(|value| Varying::of(value, ensemble, rng))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        return Value::Number(declaration.rest());
    }
    let reduce = |index: usize| {
        let mut row = columns.iter().map(|column| column.at(index));
        let first = row.next().unwrap_or(0.0);
        let value = match aggregation {
            Aggregation::Sum => first + row.sum::<f64>(),
            Aggregation::Max => row.fold(first, f64::max),
            Aggregation::Product => row.fold(first, |total, next| total * next),
            Aggregation::Min => row.fold(first, f64::min),
            Aggregation::Mean => {
                (first + row.sum::<f64>()) / f64::from(u32::try_from(columns.len()).unwrap_or(1))
            }
        };
        value / scale
    };
    if all_uniform(&columns) {
        return Value::Number(reduce(0));
    }
    per_draw(aligned(&columns, count), reduce).unwrap_or(Value::Number(0.0))
}
