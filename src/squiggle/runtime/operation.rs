use std::sync::Arc;

use crate::squiggle::{Diagnostic, Distribution, DurationValue, Value, ast::Span};

use super::Runtime;

impl Runtime {
    pub(super) fn unary(
        &mut self,
        operator: &str,
        value: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        match (operator, value) {
            ("!", Value::Boolean(value)) => Ok(Value::Boolean(!value)),
            ("!", Value::Number(value)) => Ok(Value::Boolean(value == 0.0)),
            ("-" | ".-", Value::Number(value)) => Ok(Value::Number(-value)),
            ("-", Value::Duration(value)) => duration(-value.milliseconds(), span),
            ("-" | ".-", Value::Distribution(value)) => {
                self.transform_distribution(value, |sample| -sample, span)
            }
            (_, value) => Err(Diagnostic::runtime(
                format!(
                    "operator '{operator}' does not accept {}",
                    value.type_name()
                ),
                span,
            )),
        }
    }

    pub(super) fn binary(
        &mut self,
        operator: &str,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if operator == "to" {
            let (Value::Number(low), Value::Number(high)) = (left, right) else {
                return Err(Diagnostic::runtime("'to' requires Number operands", span));
            };
            if low <= 0.0 || low >= high {
                return Err(Diagnostic::runtime("'to' requires 0 < low < high", span));
            }
            let z95 = 1.644_853_626_951_472_2;
            let mu = (low.ln() + high.ln()) / 2.0;
            let sigma = (high.ln() - low.ln()) / (2.0 * z95);
            return Distribution::lognormal(mu, sigma)
                .map(Value::Distribution)
                .map_err(|message| Diagnostic::runtime(message, span));
        }
        if matches!(
            (&left, &right),
            (Value::Date(_), Value::Date(_))
                | (Value::Date(_), Value::Duration(_))
                | (Value::Duration(_), Value::Date(_))
                | (Value::Duration(_), Value::Duration(_))
                | (Value::Duration(_), Value::Number(_))
                | (Value::Number(_), Value::Duration(_))
        ) {
            return temporal(operator, left, right, span);
        }
        match operator {
            "==" => return Ok(Value::Boolean(left == right)),
            "!=" => return Ok(Value::Boolean(left != right)),
            "&&" | "||" => return boolean(operator, left, right, span),
            "<" | "<=" | ">" | ">=" => return compare(operator, left, right, span),
            "+" if matches!((&left, &right), (Value::String(_), Value::String(_))) => {
                if let (Value::String(left), Value::String(right)) = (left, right) {
                    return Ok(Value::String(left + &right));
                }
                return Err(Diagnostic::runtime(
                    "string addition requires String operands",
                    span,
                ));
            }
            "+" if matches!((&left, &right), (Value::Array(_), Value::Array(_))) => {
                if let (Value::Array(mut left), Value::Array(right)) = (left, right) {
                    left.extend(right);
                    return Ok(Value::Array(left));
                }
                return Err(Diagnostic::runtime(
                    "array addition requires Array operands",
                    span,
                ));
            }
            _ => {}
        }
        self.numeric_binary(operator, left, right, span)
    }

    fn numeric_binary(
        &mut self,
        operator: &str,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) => scalar(operator, left, right)
                .map(Value::Number)
                .map_err(|message| Diagnostic::runtime(message, span)),
            (Value::Distribution(left), Value::Number(right)) => {
                self.combine_distribution(left, None, operator, Some(right), span)
            }
            (Value::Number(left), Value::Distribution(right)) => {
                self.combine_distribution(right, Some(left), operator, None, span)
            }
            (Value::Distribution(left), Value::Distribution(right)) => {
                self.combine_two(left, right, operator, span)
            }
            (left, right) => Err(Diagnostic::runtime(
                format!(
                    "operator '{operator}' does not accept {} and {}",
                    left.type_name(),
                    right.type_name()
                ),
                span,
            )),
        }
    }

    fn combine_distribution(
        &mut self,
        distribution: Distribution,
        left: Option<f64>,
        operator: &str,
        right: Option<f64>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let numeric = Numeric::parse(operator)
            .ok_or_else(|| Diagnostic::runtime(unknown(operator), span))?;
        let ensemble = Distribution::aligned([&distribution], self.ensemble);
        let seed = distribution.stream(&mut self.rng);
        let drawn = distribution.drawn(seed, ensemble);
        let samples: Arc<[f64]> = match (left, right) {
            (Some(left), _) => drawn.iter().map(|draw| numeric.apply(left, *draw)).collect(),
            (None, Some(right)) => drawn
                .iter()
                .map(|draw| numeric.apply(*draw, right))
                .collect(),
            (None, None) => drawn
                .iter()
                .map(|draw| numeric.apply(*draw, *draw))
                .collect(),
        };
        finished(samples, operator, span)
    }

    /// Combines two distributions elementwise at matching draw indices.
    ///
    /// Alignment is what preserves dependence: operands that resolve to the same
    /// binding share one sample set, so `x - x` cancels exactly, while operands
    /// built by separate constructors hold independent sample sets and compose as
    /// independent random variables.
    fn combine_two(
        &mut self,
        left: Distribution,
        right: Distribution,
        operator: &str,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let numeric = Numeric::parse(operator)
            .ok_or_else(|| Diagnostic::runtime(unknown(operator), span))?;
        let ensemble = Distribution::aligned([&left, &right], self.ensemble);
        let (left_seed, right_seed) = (left.stream(&mut self.rng), right.stream(&mut self.rng));
        let (left, right) = (left.drawn(left_seed, ensemble), right.drawn(right_seed, ensemble));
        let width = left.len().min(right.len());
        let samples: Arc<[f64]> = left[..width]
            .iter()
            .zip(&right[..width])
            .map(|(left, right)| numeric.apply(*left, *right))
            .collect();
        finished(samples, operator, span)
    }

    pub(super) fn transform_distribution(
        &mut self,
        distribution: Distribution,
        transform: impl Fn(f64) -> f64,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let ensemble = Distribution::aligned([&distribution], self.ensemble);
        let seed = distribution.stream(&mut self.rng);
        let drawn = distribution.drawn(seed, ensemble);
        let samples: Arc<[f64]> = drawn.iter().map(|draw| transform(*draw)).collect();
        finished(samples, "transform", span)
    }
}

/// Accepts a finished sample set, or reports that the operator left the reals.
///
/// The check is a separate pass over the array rather than a test inside the
/// loop that produced it. A branch per draw would keep that loop scalar, whereas
/// applying one arithmetic operation across a thousand draws and then asking
/// whether a thousand results are finite are both loops the compiler can run
/// several draws at a time.
fn finished(samples: Arc<[f64]>, operator: &str, span: Span) -> Result<Value, Diagnostic> {
    if !samples.iter().all(|sample| sample.is_finite()) {
        return Err(Diagnostic::runtime(non_finite(operator), span));
    }
    Distribution::from_drawn(samples)
        .map(Value::Distribution)
        .map_err(|message| Diagnostic::runtime(message, span))
}

/// A numeric operator, resolved before the loop that applies it to every draw.
///
/// Distribution algebra applies one operator across a whole sample set, so
/// matching the operator's spelling inside that loop costs a string comparison
/// per draw, of which a solve performs hundreds of millions. The spelling cannot
/// change part-way through, so it is resolved once and the loop body becomes a
/// single arithmetic instruction the compiler can keep in registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Numeric {
    Add,
    Subtract,
    Multiply,
    Divide,
    Raise,
}

impl Numeric {
    fn parse(operator: &str) -> Option<Self> {
        Some(match operator {
            "+" | ".+" => Self::Add,
            "-" | ".-" => Self::Subtract,
            "*" | ".*" => Self::Multiply,
            "/" | "./" => Self::Divide,
            "^" | ".^" => Self::Raise,
            _ => return None,
        })
    }

    #[inline]
    fn apply(self, left: f64, right: f64) -> f64 {
        match self {
            Self::Add => left + right,
            Self::Subtract => left - right,
            Self::Multiply => left * right,
            Self::Divide => left / right,
            Self::Raise => left.powf(right),
        }
    }
}

fn unknown(operator: &str) -> String {
    format!("unknown numeric operator '{operator}'")
}

fn non_finite(operator: &str) -> String {
    format!("operator '{operator}' produced a non-finite value")
}

fn scalar(operator: &str, left: f64, right: f64) -> Result<f64, String> {
    let value = Numeric::parse(operator)
        .ok_or_else(|| unknown(operator))?
        .apply(left, right);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| non_finite(operator))
}

fn boolean(operator: &str, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    let (Value::Boolean(left), Value::Boolean(right)) = (&left, &right) else {
        return Err(Diagnostic::runtime(
            format!("operator '{operator}' requires Boolean operands"),
            span,
        ));
    };
    Ok(Value::Boolean(if operator == "&&" {
        *left && *right
    } else {
        *left || *right
    }))
}

fn compare(operator: &str, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    let (Value::Number(left), Value::Number(right)) = (left, right) else {
        return Err(Diagnostic::runtime(
            format!("operator '{operator}' requires Number operands"),
            span,
        ));
    };
    Ok(Value::Boolean(match operator {
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        _ => left >= right,
    }))
}

fn temporal(operator: &str, left: Value, right: Value, span: Span) -> Result<Value, Diagnostic> {
    match (operator, left, right) {
        ("+", Value::Date(date), Value::Duration(duration))
        | ("+", Value::Duration(duration), Value::Date(date)) => {
            Ok(Value::Date(date.add(duration)))
        }
        ("-", Value::Date(left), Value::Date(right)) => Ok(Value::Duration(left.subtract(right))),
        ("-", Value::Date(date), Value::Duration(duration)) => {
            let negative = DurationValue::from_milliseconds(-duration.milliseconds())
                .map_err(|error| Diagnostic::runtime(error, span))?;
            Ok(Value::Date(date.add(negative)))
        }
        ("+", Value::Duration(left), Value::Duration(right)) => {
            duration(left.milliseconds() + right.milliseconds(), span)
        }
        ("-", Value::Duration(left), Value::Duration(right)) => {
            duration(left.milliseconds() - right.milliseconds(), span)
        }
        ("*", Value::Duration(value), Value::Number(scale))
        | ("*", Value::Number(scale), Value::Duration(value)) => {
            duration(value.milliseconds() * scale, span)
        }
        ("/", Value::Duration(left), Value::Duration(right)) => {
            Ok(Value::Number(left.milliseconds() / right.milliseconds()))
        }
        ("/", Value::Duration(value), Value::Number(divisor)) => {
            duration(value.milliseconds() / divisor, span)
        }
        (operator @ ("<" | "<=" | ">" | ">="), Value::Date(left), Value::Date(right)) => Ok(
            Value::Boolean(order(operator, left.unix_seconds(), right.unix_seconds())),
        ),
        (operator @ ("<" | "<=" | ">" | ">="), Value::Duration(left), Value::Duration(right)) => {
            Ok(Value::Boolean(order(
                operator,
                left.milliseconds(),
                right.milliseconds(),
            )))
        }
        (operator, left, right) => Err(Diagnostic::runtime(
            format!(
                "operator '{operator}' does not accept {} and {}",
                left.type_name(),
                right.type_name()
            ),
            span,
        )),
    }
}

fn duration(milliseconds: f64, span: Span) -> Result<Value, Diagnostic> {
    DurationValue::from_milliseconds(milliseconds)
        .map(Value::Duration)
        .map_err(|error| Diagnostic::runtime(error, span))
}

fn order(operator: &str, left: f64, right: f64) -> bool {
    match operator {
        "<" => left < right,
        "<=" => left <= right,
        ">" => left > right,
        _ => left >= right,
    }
}
