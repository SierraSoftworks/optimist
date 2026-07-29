use std::collections::BTreeMap;

use crate::squiggle::{Diagnostic, Distribution, Value, ast::Span};

use super::{
    Runtime,
    builtin::{arity, number},
};

builtins! {
    context(runtime, span);
        SampleSet | "SampleSet.make"(value: Number) => from_number(runtime, &[Value::Number(value)], span),
        SampleSet | "SampleSet.make"(values: [Number]) => from_list(&[Value::Array(values.into_iter().map(Value::Number).collect())], span),
        SampleSet | "SampleSet.make"(distribution: Distribution) => from_dist(runtime, &[Value::Distribution(distribution.clone())], span),
        SampleSet | "SampleSet.make"(function: Function) => from_function(runtime, &[Value::Function(function.clone())], span),
        "SampleSet.fromDist"(distribution: Distribution) => from_dist(runtime, &[Value::Distribution(distribution.clone())], span),
        "SampleSet.fromNumber"(value: Number) => from_number(runtime, &[Value::Number(value)], span),
        "SampleSet.fromList"(values: [Number]) => from_list(&[Value::Array(values.into_iter().map(Value::Number).collect())], span),
        "SampleSet.fromFn"(function: Function) => from_function(runtime, &[Value::Function(function.clone())], span),
        "SampleSet.toList"(distribution: Distribution) => to_list(runtime, &[Value::Distribution(distribution.clone())], span),
        "SampleSet.map"(distribution: Distribution, function: Function) => map(runtime, "SampleSet.map", &[Value::Distribution(distribution.clone()), Value::Function(function.clone())], span),
        "SampleSet.map2"(first: Distribution, second: Distribution, function: Function) => map(runtime, "SampleSet.map2", &[Value::Distribution(first.clone()), Value::Distribution(second.clone()), Value::Function(function.clone())], span),
        "SampleSet.map3"(first: Distribution, second: Distribution, third: Distribution, function: Function) => map(runtime, "SampleSet.map3", &[Value::Distribution(first.clone()), Value::Distribution(second.clone()), Value::Distribution(third.clone()), Value::Function(function.clone())], span),
        PointSet | "PointSet.make"(value: (Number | Distribution)) => point_set(&[value.clone()], span),
        "PointSet.fromNumber"(value: Number) => point_set(&[Value::Number(value)], span),
        "PointSet.fromDist"(distribution: Distribution) => point_set(&[Value::Distribution(distribution.clone())], span),
        "PointSet.downsample"(distribution: Distribution, count: NonNegativeInteger) => downsample(runtime, &[Value::Distribution(distribution.clone()), Value::Number(count as f64)], span),
    "PointSet.support"(distribution: Distribution) => support(&[Value::Distribution(distribution.clone())], span),
}

fn from_dist(
    runtime: &mut Runtime,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    let Value::Distribution(distribution) = &arguments[0] else {
        return Err(expected("Distribution", &arguments[0], span));
    };
    let samples = distribution.materialise(distribution.stream(&mut runtime.rng), runtime.ensemble);
    finish(Distribution::from_samples(samples), span)
}

fn from_number(runtime: &Runtime, arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    finish(
        Distribution::from_samples(vec![
            number(&arguments[0], span)?;
            runtime.config.sample_count
        ]),
        span,
    )
}

fn from_list(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    finish(
        Distribution::from_samples(
            values
                .iter()
                .map(|value| number(value, span))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        span,
    )
}

fn from_function(
    runtime: &mut Runtime,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    let Value::Function(function) = &arguments[0] else {
        return Err(expected("Function", &arguments[0], span));
    };
    let use_index = function.arity() == Some(1);
    let mut samples = Vec::with_capacity(runtime.config.sample_count);
    for index in 0..runtime.config.sample_count {
        let args = if use_index {
            vec![Value::Number(index as f64)]
        } else {
            Vec::new()
        };
        samples.push(number(
            &runtime.call(arguments[0].clone(), &args, span)?,
            span,
        )?);
    }
    finish(Distribution::from_samples(samples), span)
}

fn to_list(runtime: &mut Runtime, arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    let Value::Distribution(distribution) = &arguments[0] else {
        return Err(expected("Distribution", &arguments[0], span));
    };
    let count = Distribution::aligned([distribution], runtime.ensemble);
    let samples = distribution.materialise(distribution.stream(&mut runtime.rng), count);
    Ok(Value::Array(
        samples.into_iter().map(Value::Number).collect(),
    ))
}

fn map(
    runtime: &mut Runtime,
    name: &str,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    let distribution_count = match name {
        "SampleSet.map" => 1,
        "SampleSet.map2" => 2,
        _ => 3,
    };
    arity(arguments, distribution_count + 1, span)?;
    let function = arguments
        .last()
        .cloned()
        .ok_or_else(|| Diagnostic::runtime("SampleSet.map requires a function", span))?;
    if !matches!(function, Value::Function(_)) {
        return Err(expected("Function", &function, span));
    }
    let mut operands = Vec::new();
    for value in arguments.iter().take(distribution_count) {
        let Value::Distribution(distribution) = value else {
            return Err(expected("Distribution", value, span));
        };
        operands.push(distribution);
    }
    let count = Distribution::aligned(operands.iter().copied(), runtime.ensemble);
    let mut inputs = Vec::new();
    for distribution in operands {
        inputs.push(distribution.materialise(distribution.stream(&mut runtime.rng), count));
    }
    let count = inputs
        .iter()
        .map(Vec::len)
        .min()
        .ok_or_else(|| Diagnostic::runtime("SampleSet.map requires a distribution", span))?;
    let mut samples = Vec::with_capacity(count);
    let mut args = Vec::with_capacity(inputs.len());
    for index in 0..count {
        args.clear();
        args.extend(inputs.iter().map(|values| Value::Number(values[index])));
        samples.push(number(&runtime.call(function.clone(), &args, span)?, span)?);
    }
    finish(Distribution::from_samples(samples), span)
}

fn point_set(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    match &arguments[0] {
        value @ Value::Distribution(_) => Ok(value.clone()),
        Value::Number(value) => finish(Distribution::point(*value), span),
        value => Err(expected("Number or Distribution", value, span)),
    }
}

fn downsample(
    runtime: &mut Runtime,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(arguments, 2, span)?;
    let Value::Distribution(distribution) = &arguments[0] else {
        return Err(expected("Distribution", &arguments[0], span));
    };
    let count = number(&arguments[1], span)? as usize;
    let samples = distribution
        .sample_n(count, &mut runtime.rng)
        .map_err(|error| Diagnostic::runtime(error, span))?;
    finish(Distribution::from_samples(samples), span)
}

fn support(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 1, span)?;
    let Value::Distribution(distribution) = &arguments[0] else {
        return Err(expected("Distribution", &arguments[0], span));
    };
    let mut result = BTreeMap::new();
    let minimum = distribution
        .minimum()
        .map_err(|error| Diagnostic::runtime(error, span))?;
    let maximum = distribution
        .maximum()
        .map_err(|error| Diagnostic::runtime(error, span))?;
    let segments = if minimum.is_finite() && maximum.is_finite() {
        vec![Value::Array(vec![
            Value::Number(minimum),
            Value::Number(maximum),
        ])]
    } else {
        Vec::new()
    };
    result.insert("points".into(), Value::Array(Vec::new()));
    result.insert("segments".into(), Value::Array(segments));
    Ok(Value::dictionary(result))
}

fn finish(result: Result<Distribution, String>, span: Span) -> Result<Value, Diagnostic> {
    result
        .map(Value::Distribution)
        .map_err(|error| Diagnostic::runtime(error, span))
}
fn expected(expected: &str, value: &Value, span: Span) -> Diagnostic {
    Diagnostic::runtime(
        format!("expected {expected}, received {}", value.type_name()),
        span,
    )
}
