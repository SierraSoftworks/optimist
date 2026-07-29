use crate::squiggle::{
    Diagnostic, Value,
    ast::{BinaryOperator as Infix, Span, UnaryOperator as Prefix},
};

use super::Runtime;

builtins! {
    context(runtime, span);
    add(left: *, right: *) => runtime.binary(Infix::Add, left.clone(), right.clone(), span),
    subtract(left: *, right: *) => runtime.binary(Infix::Subtract, left.clone(), right.clone(), span),
    multiply(left: *, right: *) => runtime.binary(Infix::Multiply, left.clone(), right.clone(), span),
    divide(left: *, right: *) => runtime.binary(Infix::Divide, left.clone(), right.clone(), span),
    pow(left: *, right: *) => runtime.binary(Infix::Power, left.clone(), right.clone(), span),
    equal(left: *, right: *) => runtime.binary(Infix::Equal, left.clone(), right.clone(), span),
    unequal(left: *, right: *) => runtime.binary(Infix::NotEqual, left.clone(), right.clone(), span),
    smaller(left: *, right: *) => runtime.binary(Infix::Less, left.clone(), right.clone(), span),
    smallerEq(left: *, right: *) => runtime.binary(Infix::LessOrEqual, left.clone(), right.clone(), span),
    larger(left: *, right: *) => runtime.binary(Infix::Greater, left.clone(), right.clone(), span),
    largerEq(left: *, right: *) => runtime.binary(Infix::GreaterOrEqual, left.clone(), right.clone(), span),
    and(left: Boolean, right: Boolean) => runtime.binary(Infix::And, Value::Boolean(left), Value::Boolean(right), span),
    or(left: Boolean, right: Boolean) => runtime.binary(Infix::Or, Value::Boolean(left), Value::Boolean(right), span),
    not(value: Boolean) => runtime.unary(Prefix::Not, Value::Boolean(value), span),
    not(value: Number) => runtime.unary(Prefix::Not, Value::Number(value), span),
    unaryMinus(value: *) => runtime.unary(Prefix::Negate, value.clone(), span),
    concat(...values: String) => Ok(Value::String(values.into_iter().fold(
        String::new(),
        |mut output, value| {
            output.push_str(value);
            output
        },
    ))),
    exp(value: Number) => finite("exp", value.exp(), span),
    log(value: Number) => finite("log", value.ln(), span),
    log(value: Number, base: Number) => finite("log", value.log(base), span),
    log10(value: Number) => finite("log10", value.log10(), span),
    log2(value: Number) => finite("log2", value.log2(), span),
    floor(value: Number) => finite("floor", value.floor(), span),
    ceil(value: Number) => finite("ceil", value.ceil(), span),
    abs(value: Number) => finite("abs", value.abs(), span),
    round(value: Number) => finite("round", value.round(), span),
    mod(value: Number, modulus: Number) => {
        finite("mod", value - modulus * (value / modulus).floor(), span)
    },
    "Math.sqrt" | sqrt(value: Number) => finite("sqrt", value.sqrt(), span),
    "Math.sin" | sin(value: Number) => finite("sin", value.sin(), span),
    "Math.cos" | cos(value: Number) => finite("cos", value.cos(), span),
    "Math.tan" | tan(value: Number) => finite("tan", value.tan(), span),
    "Math.asin" | asin(value: Number) => finite("asin", value.asin(), span),
    "Math.acos" | acos(value: Number) => finite("acos", value.acos(), span),
    "Math.atan" | atan(value: Number) => finite("atan", value.atan(), span),
}

fn finite(name: &str, value: f64, span: Span) -> Result<Value, Diagnostic> {
    value
        .is_finite()
        .then_some(Value::Number(value))
        .ok_or_else(|| Diagnostic::runtime(format!("{name} produced a non-finite value"), span))
}
