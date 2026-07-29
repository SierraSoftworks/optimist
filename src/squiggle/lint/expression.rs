use std::collections::BTreeMap;

use crate::squiggle::ast::{BinaryOperator, Expression, ExpressionKind, UnaryOperator};

use super::{
    checker::Checker,
    types::{Type, Unit},
};

impl Checker {
    pub(super) fn infer(&mut self, expression: &Expression) -> Type {
        match &expression.kind {
            ExpressionKind::Number(value, unit) => {
                self.infer_number(*value, unit.as_deref(), expression.span)
            }
            ExpressionKind::Boolean(_) => Type::Boolean,
            ExpressionKind::String(_) => Type::String,
            ExpressionKind::Variable(name) => self.lookup(name).unwrap_or_else(|| {
                self.report(format!("unknown identifier '{name}'"), expression.span);
                Type::Unknown
            }),
            ExpressionKind::Array(values) => {
                let values = values
                    .iter()
                    .map(|value| self.infer(value))
                    .collect::<Vec<_>>();
                Type::Array(Box::new(homogeneous(&values)))
            }
            ExpressionKind::Dictionary(entries) => {
                let mut fields = BTreeMap::new();
                for (key, value) in entries {
                    let key_type = self.infer(key);
                    let value_type = self.infer(value);
                    if !matches!(key_type, Type::String | Type::Unknown) {
                        self.report("dictionary key must be String", key.span);
                    }
                    if let ExpressionKind::String(key) = &key.kind {
                        fields.insert(key.clone(), value_type);
                    }
                }
                Type::Dictionary(fields)
            }
            ExpressionKind::Lambda {
                parameters,
                body,
                return_unit,
            } => self.infer_function(parameters, body, return_unit.as_ref()),
            ExpressionKind::Block { statements, result } => {
                self.scopes.push(BTreeMap::new());
                for statement in statements {
                    self.check_statement(statement);
                }
                let result = self.infer(result);
                self.scopes.pop();
                result
            }
            ExpressionKind::Unary {
                operator,
                expression: value,
            } => {
                let value = self.infer(value);
                self.infer_unary(*operator, value, expression.span)
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.infer(left);
                let right = self.infer(right);
                self.infer_binary(*operator, left, right, expression.span)
            }
            ExpressionKind::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let condition_type = self.infer(condition);
                if !matches!(condition_type, Type::Boolean | Type::Unknown) {
                    self.report(
                        format!(
                            "condition must be Boolean, received {}",
                            condition_type.display_name()
                        ),
                        condition.span,
                    );
                }
                let when_true = self.infer(when_true);
                let when_false = self.infer(when_false);
                join(&when_true, &when_false)
            }
            ExpressionKind::Call {
                function,
                arguments,
            } => self.infer_call(function, arguments, expression.span),
            ExpressionKind::Lookup { value, key } => self.infer_lookup(value, key, expression.span),
            ExpressionKind::Pipe {
                value,
                function,
                arguments,
            } => {
                let mut piped = vec![value.as_ref().clone()];
                piped.extend(arguments.iter().cloned());
                self.infer_call(function, &piped, expression.span)
            }
        }
    }

    fn infer_number(
        &mut self,
        value: f64,
        suffix: Option<&str>,
        span: crate::squiggle::ast::Span,
    ) -> Type {
        match suffix {
            Some("minutes" | "hours" | "days" | "year" | "years") => Type::Duration,
            Some("n") => Type::number(Some(value * 1e-9)),
            Some("m") => Type::number(Some(value * 1e-3)),
            Some("%") => Type::number(Some(value * 1e-2)),
            Some("k") => Type::number(Some(value * 1e3)),
            Some("M") => Type::number(Some(value * 1e6)),
            Some("B" | "G") => Type::number(Some(value * 1e9)),
            Some("T") => Type::number(Some(value * 1e12)),
            Some("P") => Type::number(Some(value * 1e15)),
            Some(suffix) => {
                self.report(format!("unknown unit suffix '{suffix}'"), span);
                Type::Unknown
            }
            None => Type::number(Some(value)),
        }
    }

    pub(super) fn infer_unary(
        &mut self,
        operator: UnaryOperator,
        value: Type,
        span: crate::squiggle::ast::Span,
    ) -> Type {
        use UnaryOperator::{Negate, NegateEach, Not};
        match (operator, value) {
            (Not, Type::Boolean | Type::Number { .. }) => Type::Boolean,
            (
                Negate | NegateEach,
                value @ (Type::Number { .. } | Type::Distribution(_) | Type::Duration),
            ) => value,
            (_, Type::Unknown) => Type::Unknown,
            (operator, value) => {
                self.report(
                    format!(
                        "operator '{}' does not accept {}",
                        operator.spelling(),
                        value.display_name()
                    ),
                    span,
                );
                Type::Unknown
            }
        }
    }

    pub(super) fn infer_binary(
        &mut self,
        operator: BinaryOperator,
        left: Type,
        right: Type,
        span: crate::squiggle::ast::Span,
    ) -> Type {
        use BinaryOperator as Infix;
        if matches!(left, Type::Unknown) || matches!(right, Type::Unknown) {
            return Type::Unknown;
        }
        match operator {
            Infix::Equal | Infix::NotEqual => Type::Boolean,
            Infix::And | Infix::Or if matches!((&left, &right), (Type::Boolean, Type::Boolean)) => {
                Type::Boolean
            }
            Infix::Less | Infix::LessOrEqual | Infix::Greater | Infix::GreaterOrEqual
                if comparable(&left, &right) =>
            {
                Type::Boolean
            }
            Infix::Interval => self.numeric_result(operator, left, right, span, true),
            Infix::Add | Infix::AddEach => match (&left, &right) {
                (Type::String, Type::String) => Type::String,
                (Type::Array(left), Type::Array(right)) => Type::Array(Box::new(join(left, right))),
                (Type::Date, Type::Duration) | (Type::Duration, Type::Date) => Type::Date,
                (Type::Duration, Type::Duration) => Type::Duration,
                _ => self.numeric_result(operator, left, right, span, true),
            },
            Infix::Subtract | Infix::SubtractEach => match (&left, &right) {
                (Type::Date, Type::Date) => Type::Duration,
                (Type::Date, Type::Duration) => Type::Date,
                (Type::Duration, Type::Duration) => Type::Duration,
                _ => self.numeric_result(operator, left, right, span, true),
            },
            Infix::Multiply | Infix::MultiplyEach => match (&left, &right) {
                (Type::Duration, Type::Number { .. }) | (Type::Number { .. }, Type::Duration) => {
                    Type::Duration
                }
                _ => self.numeric_result(operator, left, right, span, false),
            },
            Infix::Divide | Infix::DivideEach => match (&left, &right) {
                (Type::Duration, Type::Duration) => Type::number(None),
                (Type::Duration, Type::Number { .. }) => Type::Duration,
                _ => self.numeric_result(operator, left, right, span, false),
            },
            Infix::Power | Infix::PowerEach => self.power_result(left, right, span),
            _ => {
                self.report(
                    format!(
                        "operator '{}' does not accept {} and {}",
                        operator.spelling(),
                        left.display_name(),
                        right.display_name()
                    ),
                    span,
                );
                Type::Unknown
            }
        }
    }

    fn numeric_result(
        &mut self,
        operator: BinaryOperator,
        left: Type,
        right: Type,
        span: crate::squiggle::ast::Span,
        same_unit: bool,
    ) -> Type {
        let Some((left_unit, left_dist)) = numeric_unit(&left) else {
            return self.binary_error(operator, left, right, span);
        };
        let Some((right_unit, right_dist)) = numeric_unit(&right) else {
            return self.binary_error(operator, left, right, span);
        };
        if same_unit && left_unit != right_unit {
            self.report(
                format!(
                    "operator '{}' combines incompatible units {left_unit} and {right_unit}",
                    operator.spelling()
                ),
                span,
            );
            return Type::Unknown;
        }
        let unit = if same_unit {
            left_unit
        } else if matches!(
            operator,
            BinaryOperator::Divide | BinaryOperator::DivideEach
        ) {
            left_unit.combine(&right_unit, -1.0)
        } else {
            left_unit.combine(&right_unit, 1.0)
        };
        if left_dist || right_dist || operator == BinaryOperator::Interval {
            Type::Distribution(unit)
        } else {
            Type::Number {
                literal: None,
                unit,
            }
        }
    }

    fn power_result(&mut self, left: Type, right: Type, span: crate::squiggle::ast::Span) -> Type {
        let Some((unit, distribution)) = numeric_unit(&left) else {
            return self.binary_error(BinaryOperator::Power, left, right, span);
        };
        let Type::Number {
            literal,
            unit: exponent_unit,
        } = right
        else {
            return self.binary_error(BinaryOperator::Power, left, right, span);
        };
        if exponent_unit != Unit::default() {
            self.report("power exponent must be dimensionless", span);
            return Type::Unknown;
        }
        let unit = literal.map_or(Unit::default(), |literal| unit.pow(literal));
        if distribution {
            Type::Distribution(unit)
        } else {
            Type::Number {
                literal: None,
                unit,
            }
        }
    }

    fn binary_error(
        &mut self,
        operator: BinaryOperator,
        left: Type,
        right: Type,
        span: crate::squiggle::ast::Span,
    ) -> Type {
        self.report(
            format!(
                "operator '{}' does not accept {} and {}",
                operator.spelling(),
                left.display_name(),
                right.display_name()
            ),
            span,
        );
        Type::Unknown
    }
}

fn homogeneous(values: &[Type]) -> Type {
    let Some(first) = values.first() else {
        return Type::Unknown;
    };
    values
        .iter()
        .skip(1)
        .fold(first.clone(), |current, value| join(&current, value))
}

pub(super) fn join(left: &Type, right: &Type) -> Type {
    if left == right {
        return left.clone();
    }
    if matches!(left, Type::Unknown) || matches!(right, Type::Unknown) {
        return Type::Unknown;
    }
    let mut values = Vec::new();
    for value in [left, right] {
        if let Type::Union(nested) = value {
            values.extend(nested.clone());
        } else if !values.contains(value) {
            values.push(value.clone());
        }
    }
    Type::Union(values)
}

fn numeric_unit(value: &Type) -> Option<(Unit, bool)> {
    match value {
        Type::Number { unit, .. } => Some((unit.clone(), false)),
        Type::Distribution(unit) => Some((unit.clone(), true)),
        _ => None,
    }
}

fn comparable(left: &Type, right: &Type) -> bool {
    match (left, right) {
        (Type::Number { unit: left, .. }, Type::Number { unit: right, .. }) => left == right,
        (Type::Date, Type::Date) | (Type::Duration, Type::Duration) => true,
        _ => false,
    }
}
