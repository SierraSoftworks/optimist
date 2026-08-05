use std::collections::BTreeMap;

use crate::squiggle::ast::{BinaryOperator, Expression, ExpressionKind, Span, UnaryOperator};

use super::{
    BuiltinSignature,
    checker::Checker,
    types::{FunctionType, Type, Unit},
};

impl Checker {
    pub(super) fn infer_call(
        &mut self,
        function: &Expression,
        arguments: &[Expression],
        span: Span,
    ) -> Type {
        let argument_types = arguments
            .iter()
            .map(|argument| self.infer(argument))
            .collect::<Vec<_>>();
        let function_type = if let Some(name) = Self::expression_name(function) {
            if self.builtins.contains(name.as_str()) {
                Type::Builtin(name)
            } else {
                self.infer(function)
            }
        } else {
            self.infer(function)
        };
        match function_type {
            Type::Builtin(name) => self.infer_builtin(&name, &argument_types, span),
            Type::Function(function) => self.infer_user_call(&function, &argument_types, span),
            Type::Unknown => Type::Unknown,
            value => {
                self.report(
                    format!("cannot call {} value", value.display_name()),
                    function.span,
                );
                Type::Unknown
            }
        }
    }

    pub(super) fn infer_lookup(
        &mut self,
        value: &Expression,
        key: &Expression,
        span: Span,
    ) -> Type {
        if let Some(name) = Self::expression_name(&Expression {
            kind: ExpressionKind::Lookup {
                value: Box::new(value.clone()),
                key: Box::new(key.clone()),
            },
            span,
        }) {
            if self.builtins.contains(name.as_str()) {
                return Type::Builtin(name);
            }
            if let Some((namespace, _)) = name.split_once('.')
                && self
                    .builtins
                    .iter()
                    .any(|builtin| builtin.starts_with(&format!("{namespace}.")))
            {
                self.report(format!("unknown builtin '{name}'"), span);
                return Type::Unknown;
            }
        }
        let value_type = self.infer(value);
        let key_type = self.infer(key);
        match value_type {
            Type::Dictionary(fields) => {
                if !matches!(key_type, Type::String | Type::Unknown) {
                    self.report("dictionary lookup key must be String", key.span);
                    return Type::Unknown;
                }
                if let ExpressionKind::String(key) = &key.kind {
                    fields.get(key).cloned().unwrap_or_else(|| {
                        self.report(format!("dictionary has no known key '{key}'"), span);
                        Type::Unknown
                    })
                } else {
                    Type::Unknown
                }
            }
            Type::Array(element) => {
                if !matches!(key_type, Type::Number { .. } | Type::Unknown) {
                    self.report("array index must be Number", key.span);
                }
                *element
            }
            Type::Unknown => Type::Unknown,
            value => {
                self.report(format!("cannot index {} value", value.display_name()), span);
                Type::Unknown
            }
        }
    }

    fn infer_builtin(&mut self, name: &str, arguments: &[Type], span: Span) -> Type {
        let candidates = self
            .signatures
            .iter()
            .filter(|signature| {
                signature.names.contains(&name) && signature_matches(signature, arguments)
            })
            .cloned()
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            let expected = self
                .signatures
                .iter()
                .filter(|signature| signature.names.contains(&name))
                .map(|signature| format_signature(name, signature))
                .collect::<Vec<_>>()
                .join(" or ");
            let received = arguments
                .iter()
                .map(Type::display_name)
                .collect::<Vec<_>>()
                .join(", ");
            self.report(
                format!("no overload of '{name}' accepts ({received}); expected {expected}"),
                span,
            );
            return Type::Unknown;
        }
        if let [left, right] = arguments {
            let operator = match name {
                "add" => Some(BinaryOperator::Add),
                "subtract" => Some(BinaryOperator::Subtract),
                "multiply" => Some(BinaryOperator::Multiply),
                "divide" => Some(BinaryOperator::Divide),
                "pow" => Some(BinaryOperator::Power),
                "equal" => Some(BinaryOperator::Equal),
                "unequal" => Some(BinaryOperator::NotEqual),
                "smaller" => Some(BinaryOperator::Less),
                "smallerEq" => Some(BinaryOperator::LessOrEqual),
                "larger" => Some(BinaryOperator::Greater),
                "largerEq" => Some(BinaryOperator::GreaterOrEqual),
                "and" => Some(BinaryOperator::And),
                "or" => Some(BinaryOperator::Or),
                _ => None,
            };
            if let Some(operator) = operator {
                return self.infer_binary(operator, left.clone(), right.clone(), span);
            }
        }
        if let [value] = arguments
            && matches!(name, "not" | "unaryMinus")
        {
            let operator = if name == "not" {
                UnaryOperator::Not
            } else {
                UnaryOperator::Negate
            };
            return self.infer_unary(operator, value.clone(), span);
        }
        builtin_result(name, arguments)
    }

    fn infer_user_call(&mut self, function: &FunctionType, arguments: &[Type], span: Span) -> Type {
        if function.parameters.len() != arguments.len() {
            let noun = if function.parameters.len() == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.report(
                format!(
                    "function expects {} {}, received {}",
                    function.parameters.len(),
                    noun,
                    arguments.len()
                ),
                span,
            );
            return Type::Unknown;
        }
        for (index, (expected, received)) in function.parameters.iter().zip(arguments).enumerate() {
            if !compatible(expected, received) {
                self.report(
                    format!(
                        "argument {} expects {}, received {}",
                        index + 1,
                        expected.display_name(),
                        received.display_name()
                    ),
                    span,
                );
            }
        }
        function.result.as_ref().clone()
    }
}

fn signature_matches(signature: &BuiltinSignature, arguments: &[Type]) -> bool {
    if signature.variadic.is_none() && arguments.len() != signature.parameters.len() {
        return false;
    }
    if signature.variadic.is_some() && arguments.len() < signature.parameters.len() {
        return false;
    }
    if !signature
        .parameters
        .iter()
        .zip(arguments)
        .all(|(parameter, value)| value.accepts(&parameter.constraint))
    {
        return false;
    }
    signature.variadic.as_ref().is_none_or(|variadic| {
        arguments[signature.parameters.len()..]
            .iter()
            .all(|value| value.accepts(&variadic.constraint))
    })
}

fn format_signature(name: &str, signature: &BuiltinSignature) -> String {
    let mut parameters = signature
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name, parameter.constraint))
        .collect::<Vec<_>>();
    if let Some(variadic) = &signature.variadic {
        parameters.push(format!("...{}: {}", variadic.name, variadic.constraint));
    }
    format!("{name}({})", parameters.join(", "))
}

fn compatible(expected: &Type, received: &Type) -> bool {
    if matches!(expected, Type::Unknown) || matches!(received, Type::Unknown) {
        return true;
    }
    match (expected, received) {
        (Type::Number { unit: left, .. }, Type::Number { unit: right, .. }) => left == right,
        (Type::Distribution(left), Type::Distribution(right)) => left == right,
        (Type::Array(left), Type::Array(right)) => compatible(left, right),
        _ => expected.display_name() == received.display_name(),
    }
}

fn builtin_result(name: &str, arguments: &[Type]) -> Type {
    match name {
        "typeOf" | "concat" | "String.make" => Type::String,
        "String.split" => Type::Array(Box::new(Type::String)),
        "inspect" => arguments.first().cloned().unwrap_or(Type::Unknown),
        "try" => Type::Dictionary(
            [
                ("tag".into(), Type::String),
                ("value".into(), Type::Unknown),
            ]
            .into(),
        ),
        "throw" => Type::Unknown,
        "equal" | "unequal" | "smaller" | "smallerEq" | "larger" | "largerEq" | "and" | "or"
        | "not" => Type::Boolean,
        "add" | "subtract" | "multiply" | "divide" | "pow" | "unaryMinus" => {
            arguments.first().cloned().unwrap_or(Type::Unknown)
        }
        "exp" | "log" | "log10" | "log2" | "floor" | "ceil" | "abs" | "round" | "mod" | "sqrt"
        | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "Math.sqrt" | "Math.sin"
        | "Math.cos" | "Math.tan" | "Math.asin" | "Math.acos" | "Math.atan" => Type::number(None),
        "normal" | "lognormal" | "uniform" | "beta" | "cauchy" | "gamma" | "logistic"
        | "exponential" | "bernoulli" | "binomial" | "poisson" | "triangular" | "pointMass"
        | "mixture" | "mx" => Type::Distribution(Unit::default()),
        name if name.starts_with("Dist.") || name.starts_with("Sym.") => distribution_result(name),
        "cdf" | "pdf" | "inv" | "sample" | "mean" | "median" | "quantile" | "stdev"
        | "variance" | "min" | "max" | "mode" => Type::number(None),
        "sampleN" => Type::Array(Box::new(Type::number(None))),
        "sum" | "product" => arguments
            .first()
            .and_then(|value| match value {
                Type::Array(element) => Some(element.as_ref().clone()),
                _ => None,
            })
            .unwrap_or(Type::Unknown),
        "sort" | "cumsum" | "cumprod" | "diff" => Type::Array(Box::new(Type::number(None))),
        name if name.starts_with("List.") => list_result(name, arguments),
        name if name.starts_with("Dict.") => dict_result(name, arguments),
        "SampleSet" | "PointSet" => Type::Distribution(Unit::default()),
        name if name.starts_with("SampleSet.") || name.starts_with("PointSet.") => {
            sample_result(name)
        }
        "Number.rangeDomain" | "Date.rangeDomain" => Type::Domain,
        "Date.make" | "Date.fromUnixTime" => Type::Date,
        "Date.toUnixTime" | "System.sampleCount" => Type::number(None),
        name if name.contains("Duration.from")
            || matches!(name, "fromMinutes" | "fromHours" | "fromDays" | "fromYears") =>
        {
            Type::Duration
        }
        name if name.contains("Duration.to")
            || matches!(name, "toMinutes" | "toHours" | "toDays" | "toYears") =>
        {
            Type::number(None)
        }
        _ => Type::Unknown,
    }
}

fn distribution_result(name: &str) -> Type {
    match name.rsplit('.').next() {
        Some("cdf" | "pdf" | "inv" | "sample" | "klDivergence" | "logScore") => Type::number(None),
        Some("sampleN") => Type::Array(Box::new(Type::number(None))),
        _ => Type::Distribution(Unit::default()),
    }
}

fn list_result(name: &str, arguments: &[Type]) -> Type {
    match name {
        "List.length" | "List.findIndex" => Type::number(None),
        "List.every" | "List.some" => Type::Boolean,
        "List.first" | "List.last" | "List.find" | "List.minBy" | "List.maxBy" | "List.sample" => {
            array_element(arguments)
        }
        "List.reduce" | "List.reduceReverse" | "List.reduceWhile" => {
            arguments.get(1).cloned().unwrap_or(Type::Unknown)
        }
        "List.map" => Type::Array(Box::new(Type::Unknown)),
        "List.zip" | "List.unzip" => Type::Array(Box::new(Type::Array(Box::new(Type::Unknown)))),
        _ => arguments
            .first()
            .cloned()
            .unwrap_or(Type::Array(Box::new(Type::Unknown))),
    }
}

fn dict_result(name: &str, arguments: &[Type]) -> Type {
    match name {
        "Dict.has" => Type::Boolean,
        "Dict.size" => Type::number(None),
        "Dict.keys" => Type::Array(Box::new(Type::String)),
        "Dict.values" | "Dict.toList" => Type::Array(Box::new(Type::Unknown)),
        _ => arguments
            .first()
            .cloned()
            .unwrap_or(Type::Dictionary(BTreeMap::new())),
    }
}

fn sample_result(name: &str) -> Type {
    match name {
        "SampleSet.toList" => Type::Array(Box::new(Type::number(None))),
        "PointSet.support" => Type::Dictionary(BTreeMap::new()),
        _ => Type::Distribution(Unit::default()),
    }
}

fn array_element(arguments: &[Type]) -> Type {
    arguments
        .first()
        .and_then(|value| match value {
            Type::Array(element) => Some(element.as_ref().clone()),
            _ => None,
        })
        .unwrap_or(Type::Unknown)
}
