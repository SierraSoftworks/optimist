use std::sync::Arc;

use crate::squiggle::{
    Diagnostic, Distribution, DurationValue, Value,
    ast::{BinaryOperator, Span, UnaryOperator},
};

use super::Runtime;

impl Runtime {
    pub(super) fn unary(
        &mut self,
        operator: UnaryOperator,
        value: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        use UnaryOperator::{Negate, NegateEach, Not};
        match (operator, value) {
            (Not, Value::Boolean(value)) => Ok(Value::Boolean(!value)),
            (Not, Value::Number(value)) => Ok(Value::Boolean(value == 0.0)),
            (Negate | NegateEach, Value::Number(value)) => Ok(Value::Number(-value)),
            (Negate, Value::Duration(value)) => duration(-value.milliseconds(), span),
            (Negate | NegateEach, Value::Distribution(value)) => {
                self.transform_distribution(value, |sample| -sample, span)
            }
            (operator, value) => Err(Diagnostic::runtime(
                format!(
                    "operator '{}' does not accept {}",
                    operator.spelling(),
                    value.type_name()
                ),
                span,
            )),
        }
    }

    pub(super) fn binary(
        &mut self,
        operator: BinaryOperator,
        left: Value,
        right: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        use BinaryOperator::{
            Add, And, Equal, Greater, GreaterOrEqual, Interval, Less, LessOrEqual, NotEqual, Or,
        };
        if operator == Interval {
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
            Equal => return Ok(Value::Boolean(left == right)),
            NotEqual => return Ok(Value::Boolean(left != right)),
            And | Or => return boolean(operator, left, right, span),
            Less | LessOrEqual | Greater | GreaterOrEqual => {
                return compare(operator, left, right, span);
            }
            Add if matches!((&left, &right), (Value::String(_), Value::String(_))) => {
                if let (Value::String(left), Value::String(right)) = (left, right) {
                    return Ok(Value::String(left + &right));
                }
                return Err(Diagnostic::runtime(
                    "string addition requires String operands",
                    span,
                ));
            }
            Add if matches!((&left, &right), (Value::Array(_), Value::Array(_))) => {
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
        operator: BinaryOperator,
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
                    "operator '{}' does not accept {} and {}",
                    operator.spelling(),
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
        operator: BinaryOperator,
        right: Option<f64>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let numeric =
            Numeric::parse(operator).ok_or_else(|| Diagnostic::runtime(unknown(operator), span))?;
        let ensemble = Distribution::aligned([&distribution], self.ensemble);
        let seed = distribution.stream(&mut self.rng);
        let drawn = distribution.drawn(seed, ensemble);
        let samples: Arc<[f64]> = match (left, right) {
            (Some(left), _) => drawn
                .iter()
                .map(|draw| numeric.apply(left, *draw))
                .collect(),
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
        operator: BinaryOperator,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let numeric =
            Numeric::parse(operator).ok_or_else(|| Diagnostic::runtime(unknown(operator), span))?;
        let ensemble = Distribution::aligned([&left, &right], self.ensemble);
        let (left_seed, right_seed) = (left.stream(&mut self.rng), right.stream(&mut self.rng));
        let (left, right) = (
            left.drawn(left_seed, ensemble),
            right.drawn(right_seed, ensemble),
        );
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
        finished(samples, BinaryOperator::Multiply, span)
    }
}

/// Accepts a finished sample set, or reports that the operator left the reals.
///
/// The check is a separate pass over the array rather than a test inside the
/// loop that produced it. A branch per draw would keep that loop scalar, whereas
/// applying one arithmetic operation across a thousand draws and then asking
/// whether a thousand results are finite are both loops the compiler can run
/// several draws at a time.
fn finished(
    samples: Arc<[f64]>,
    operator: BinaryOperator,
    span: Span,
) -> Result<Value, Diagnostic> {
    if !samples.iter().all(|sample| sample.is_finite()) {
        return Err(Diagnostic::runtime(non_finite(operator), span));
    }
    Distribution::from_drawn(samples)
        .map(Value::Distribution)
        .map_err(|message| Diagnostic::runtime(message, span))
}

/// The arithmetic an operator performs on one pair of draws.
///
/// Distribution algebra applies one operator across a whole sample set, so the
/// choice is made once outside the loop and the loop body becomes a single
/// arithmetic instruction the compiler can keep in registers. The elementwise
/// spellings collapse into the plain ones here: they differ only in the operands
/// they accept, which has already been decided by the time a draw is reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Numeric {
    Add,
    Subtract,
    Multiply,
    Divide,
    Raise,
}

impl Numeric {
    fn parse(operator: BinaryOperator) -> Option<Self> {
        use BinaryOperator as Operator;
        Some(match operator {
            Operator::Add | Operator::AddEach => Self::Add,
            Operator::Subtract | Operator::SubtractEach => Self::Subtract,
            Operator::Multiply | Operator::MultiplyEach => Self::Multiply,
            Operator::Divide | Operator::DivideEach => Self::Divide,
            Operator::Power | Operator::PowerEach => Self::Raise,
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

fn unknown(operator: BinaryOperator) -> String {
    format!("unknown numeric operator '{}'", operator.spelling())
}

fn non_finite(operator: BinaryOperator) -> String {
    format!(
        "operator '{}' produced a non-finite value",
        operator.spelling()
    )
}

fn scalar(operator: BinaryOperator, left: f64, right: f64) -> Result<f64, String> {
    let value = Numeric::parse(operator)
        .ok_or_else(|| unknown(operator))?
        .apply(left, right);
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| non_finite(operator))
}

fn boolean(
    operator: BinaryOperator,
    left: Value,
    right: Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    let (Value::Boolean(left), Value::Boolean(right)) = (&left, &right) else {
        return Err(Diagnostic::runtime(
            format!(
                "operator '{}' requires Boolean operands",
                operator.spelling()
            ),
            span,
        ));
    };
    Ok(Value::Boolean(if operator == BinaryOperator::And {
        *left && *right
    } else {
        *left || *right
    }))
}

fn compare(
    operator: BinaryOperator,
    left: Value,
    right: Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    let (Value::Number(left), Value::Number(right)) = (left, right) else {
        return Err(Diagnostic::runtime(
            format!(
                "operator '{}' requires Number operands",
                operator.spelling()
            ),
            span,
        ));
    };
    Ok(Value::Boolean(order(operator, left, right)))
}

fn temporal(
    operator: BinaryOperator,
    left: Value,
    right: Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    use BinaryOperator::{
        Add, Divide, Greater, GreaterOrEqual, Less, LessOrEqual, Multiply, Subtract,
    };
    match (operator, left, right) {
        (Add, Value::Date(date), Value::Duration(duration))
        | (Add, Value::Duration(duration), Value::Date(date)) => {
            Ok(Value::Date(date.add(duration)))
        }
        (Subtract, Value::Date(left), Value::Date(right)) => {
            Ok(Value::Duration(left.subtract(right)))
        }
        (Subtract, Value::Date(date), Value::Duration(duration)) => {
            let negative = DurationValue::from_milliseconds(-duration.milliseconds())
                .map_err(|error| Diagnostic::runtime(error, span))?;
            Ok(Value::Date(date.add(negative)))
        }
        (Add, Value::Duration(left), Value::Duration(right)) => {
            duration(left.milliseconds() + right.milliseconds(), span)
        }
        (Subtract, Value::Duration(left), Value::Duration(right)) => {
            duration(left.milliseconds() - right.milliseconds(), span)
        }
        (Multiply, Value::Duration(value), Value::Number(scale))
        | (Multiply, Value::Number(scale), Value::Duration(value)) => {
            duration(value.milliseconds() * scale, span)
        }
        (Divide, Value::Duration(left), Value::Duration(right)) => {
            Ok(Value::Number(left.milliseconds() / right.milliseconds()))
        }
        (Divide, Value::Duration(value), Value::Number(divisor)) => {
            duration(value.milliseconds() / divisor, span)
        }
        (
            operator @ (Less | LessOrEqual | Greater | GreaterOrEqual),
            Value::Date(left),
            Value::Date(right),
        ) => Ok(Value::Boolean(order(
            operator,
            left.unix_seconds(),
            right.unix_seconds(),
        ))),
        (
            operator @ (Less | LessOrEqual | Greater | GreaterOrEqual),
            Value::Duration(left),
            Value::Duration(right),
        ) => Ok(Value::Boolean(order(
            operator,
            left.milliseconds(),
            right.milliseconds(),
        ))),
        (operator, left, right) => Err(Diagnostic::runtime(
            format!(
                "operator '{}' does not accept {} and {}",
                operator.spelling(),
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

fn order(operator: BinaryOperator, left: f64, right: f64) -> bool {
    match operator {
        BinaryOperator::Less => left < right,
        BinaryOperator::LessOrEqual => left <= right,
        BinaryOperator::Greater => left > right,
        _ => left >= right,
    }
}
