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
        let ensemble = Distribution::aligned([&distribution], self.ensemble);
        let seed = distribution.stream(&mut self.rng);
        let drawn = distribution.drawn(seed, ensemble);
        let samples = drawn
            .iter()
            .map(|draw| scalar(operator, left.unwrap_or(*draw), right.unwrap_or(*draw)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|message| Diagnostic::runtime(message, span))?;
        Distribution::from_samples(samples)
            .map(Value::Distribution)
            .map_err(|message| Diagnostic::runtime(message, span))
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
        let ensemble = Distribution::aligned([&left, &right], self.ensemble);
        let (left_seed, right_seed) = (left.stream(&mut self.rng), right.stream(&mut self.rng));
        let (left, right) = (left.drawn(left_seed, ensemble), right.drawn(right_seed, ensemble));
        let samples = left
            .iter()
            .zip(right.iter())
            .map(|(left, right)| scalar(operator, *left, *right))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|message| Diagnostic::runtime(message, span))?;
        Distribution::from_samples(samples)
            .map(Value::Distribution)
            .map_err(|message| Diagnostic::runtime(message, span))
    }

    pub(super) fn transform_distribution(
        &mut self,
        distribution: Distribution,
        transform: impl Fn(f64) -> f64,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let ensemble = Distribution::aligned([&distribution], self.ensemble);
        let seed = distribution.stream(&mut self.rng);
        let samples = distribution
            .drawn(seed, ensemble)
            .iter()
            .map(|draw| transform(*draw))
            .collect();
        Distribution::from_samples(samples)
            .map(Value::Distribution)
            .map_err(|message| Diagnostic::runtime(message, span))
    }
}

fn scalar(operator: &str, left: f64, right: f64) -> Result<f64, String> {
    let value = match operator {
        "+" | ".+" => left + right,
        "-" | ".-" => left - right,
        "*" | ".*" => left * right,
        "/" | "./" => left / right,
        "^" | ".^" => left.powf(right),
        _ => return Err(format!("unknown numeric operator '{operator}'")),
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("operator '{operator}' produced a non-finite value"))
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
