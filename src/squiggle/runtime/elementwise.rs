//! Elementwise evaluation of scalar formulas over aligned sample sets.
//!
//! System-design formulas are scalar functions of several quantities, and every
//! one of those quantities may be uncertain. Evaluating such a formula at the
//! means of its inputs is wrong whenever the formula is non-linear, which is the
//! usual case: by Jensen's inequality $\mathbb{E}[f(X)] \neq f(\mathbb{E}[X])$
//! for any strictly convex or concave $f$. Queueing delay is strongly convex in
//! utilisation, so a model evaluated at mean utilisation systematically
//! understates delay and can miss saturation entirely.
//!
//! Formulas are therefore applied per draw. Each argument contributes either a
//! constant or a sample set, all sample sets are aligned to a common draw count,
//! and the scalar formula runs once per index:
//!
//! $$f(X_1, \dots, X_m)_i = f(x_{1,i}, \dots, x_{m,i})$$
//!
//! Because alignment reuses the shared draws described in
//! [`crate::squiggle::Distribution`], dependence between arguments is preserved.
//! Two arguments derived from one upstream quantity vary together at each index
//! rather than being combined as if independent, which is what keeps a feedback
//! path from inventing or destroying variance.
//!
//! The result is a sample set rather than a symbolic family. Its accuracy is
//! bounded by the configured draw count, and tail statistics are the least
//! accurate part of it, so quantiles far beyond the draw count's resolution must
//! not be presented as exact.

use crate::squiggle::{Diagnostic, Distribution, Value, ast::Span};

use super::{Runtime, builtin::number};

use crate::profile::count;

enum Column {
    Constant(f64),
    /// This value's share, resolved once so the loop below indexes memory.
    Drawn(std::sync::Arc<[f64]>),
}

impl Column {
    fn at(&self, index: usize) -> f64 {
        match self {
            Self::Constant(value) => *value,
            Self::Drawn(draws) => draws.get(index).copied().unwrap_or(f64::NAN),
        }
    }
}

/// Applies a scalar formula across the aligned draws of its arguments.
///
/// Returns a plain number when every argument is certain, and a sample set as
/// soon as any argument is uncertain. `compute` receives one value per argument
/// in the order supplied and returns a domain error message when the formula has
/// no finite value for that combination.
///
/// Draws are independent, so this loop looks like somewhere to spread work
/// across threads, and it is not. A solve calls this tens of thousands of times
/// with a sample set of a thousand and a formula of two or three arithmetic
/// operations; splitting that costs several times what evaluating it costs.
/// Measured on the shipped examples, doing so made a solve five times slower at
/// a thousand draws and was still losing at ten thousand. Parallelism belongs
/// where the unit of work is a whole relaxation, not a single formula.
pub(super) fn elementwise(
    runtime: &mut Runtime,
    arguments: &[Value],
    span: Span,
    compute: impl Fn(&[f64]) -> Result<f64, String>,
) -> Result<Value, Diagnostic> {
    let fail = |message: String| Diagnostic::runtime(message, span);
    count!(Elementwise);
    if !arguments
        .iter()
        .any(|argument| matches!(argument, Value::Distribution(_)))
    {
        let row = arguments
            .iter()
            .map(|argument| number(argument, span))
            .collect::<Result<Vec<_>, _>>()?;
        return compute(&row).map(Value::Number).map_err(fail);
    }

    let ensemble = Distribution::aligned(
        arguments.iter().filter_map(|argument| match argument {
            Value::Distribution(distribution) => Some(distribution),
            _ => None,
        }),
        runtime.ensemble,
    );
    let mut columns = Vec::with_capacity(arguments.len());
    for argument in arguments {
        columns.push(match argument {
            Value::Distribution(distribution) => {
                let seed = distribution.stream(&mut runtime.rng);
                Column::Drawn(distribution.drawn(seed, ensemble))
            }
            value => Column::Constant(number(value, span)?),
        });
    }

    // The columns already hold this runtime's share, so the formula runs across
    // the draws that share owns rather than the whole ensemble they came from.
    let width = ensemble.len();

    let mut row = vec![0.0; columns.len()];
    count!(Draws, width);
    let samples = gathered(width, |index| {
        for (slot, column) in row.iter_mut().zip(&columns) {
            *slot = column.at(index);
        }
        compute(&row)
    })
    .map_err(&fail)?;
    Distribution::from_drawn(samples)
        .map(Value::Distribution)
        .map_err(fail)
}

/// Gathers `width` draws into the array that will hold them, in one allocation.
///
/// The obvious spelling collects into a `Vec` and hands that to
/// [`Distribution::from_samples`], which allocates a second time and copies
/// every draw across. Collecting straight into the shared array avoids both, but
/// the standard library builds one only from an infallible iterator of known
/// length. The failure is therefore carried out of the loop in a slot rather
/// than short-circuiting it: the loop runs to the end and the array is discarded
/// unread, which costs a few arithmetic operations on the path where the caller
/// is about to be handed an error anyway.
///
/// A non-finite draw is a failure here rather than in a later scan, because the
/// draw that produced it is the only place that can say which one it was.
pub(super) fn gathered(
    width: usize,
    mut draw: impl FnMut(usize) -> Result<f64, String>,
) -> Result<std::sync::Arc<[f64]>, String> {
    let mut failure: Option<String> = None;
    let samples: std::sync::Arc<[f64]> = (0..width)
        .map(|index| match draw(index) {
            Ok(sample) if sample.is_finite() => sample,
            Ok(_) => {
                failure.get_or_insert_with(|| "a draw has no finite value".to_owned());
                0.0
            }
            Err(message) => {
                failure.get_or_insert(message);
                0.0
            }
        })
        .collect();
    match failure {
        Some(message) => Err(message),
        None => Ok(samples),
    }
}

/// Rejects a formula result that has left the representable domain.
pub(super) fn finite(value: f64, what: &str) -> Result<f64, String> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("{what} has no finite value for these inputs"))
}

#[cfg(test)]
mod tests {
    use crate::squiggle::{Runtime, RuntimeConfig};

    use super::*;

    fn runtime() -> Runtime {
        Runtime::with_config(RuntimeConfig {
            seed: 11,
            sample_count: 512,
            max_steps: 100_000,
        })
        .expect("runtime")
    }

    fn distribution(source: &str) -> Value {
        runtime().evaluate(source).expect("evaluates")
    }

    #[test]
    fn certain_arguments_yield_a_plain_number() -> Result<(), Diagnostic> {
        let arguments = [Value::Number(3.0), Value::Number(4.0)];
        let result = elementwise(&mut runtime(), &arguments, Span::default(), |row| {
            Ok(row[0] * row[1])
        })?;
        assert_eq!(result, Value::Number(12.0));
        Ok(())
    }

    #[test]
    fn uncertain_arguments_yield_a_sample_set() -> Result<(), Diagnostic> {
        let arguments = [distribution("uniform(1, 3)"), Value::Number(2.0)];
        let result = elementwise(&mut runtime(), &arguments, Span::default(), |row| {
            Ok(row[0] * row[1])
        })?;
        let Value::Distribution(result) = result else {
            panic!("expected a distribution");
        };
        assert!((result.mean().expect("mean") - 4.0).abs() < 0.05);
        Ok(())
    }

    #[test]
    fn shared_inputs_stay_aligned_across_arguments() -> Result<(), Diagnostic> {
        let mut runtime = runtime();
        let value = runtime.evaluate("x = uniform(1, 5)\nx").expect("evaluates");
        let arguments = [value.clone(), value];
        let result = elementwise(&mut runtime, &arguments, Span::default(), |row| {
            Ok(row[0] - row[1])
        })?;
        let Value::Distribution(result) = result else {
            panic!("expected a distribution");
        };
        assert_eq!(result.stdev().expect("stdev"), 0.0);
        Ok(())
    }

    #[test]
    fn a_convex_formula_differs_from_evaluation_at_the_mean() -> Result<(), Diagnostic> {
        let arguments = [distribution("uniform(0.1, 0.9)")];
        let result = elementwise(&mut runtime(), &arguments, Span::default(), |row| {
            Ok(1.0 / (1.0 - row[0]))
        })?;
        let Value::Distribution(result) = result else {
            panic!("expected a distribution");
        };
        let at_the_mean = 1.0 / (1.0 - 0.5);
        assert!(
            result.mean().expect("mean") > at_the_mean + 0.1,
            "Jensen's inequality must be visible, got {}",
            result.mean().expect("mean")
        );
        Ok(())
    }

    #[test]
    fn domain_errors_are_reported() {
        let arguments = [Value::Number(1.0)];
        let result = elementwise(&mut runtime(), &arguments, Span::default(), |_| {
            Err("undefined".to_owned())
        });
        assert!(result.is_err());
    }
}
