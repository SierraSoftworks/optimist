use std::collections::BTreeMap;

use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::{
    Runtime,
    builtin::{arity, number},
    elementwise::elementwise,
};

builtins! {
    context(runtime, span);
    sum(values: Array) => fold(runtime, "sum", vec![Value::Array(values.clone())], span),
    product(values: Array) => fold(runtime, "product", vec![Value::Array(values.clone())], span),
    mean(value: Distribution) => statistic("mean", vec![Value::Distribution(value.clone())], span),
    mean(values: [Number]) => numeric_statistic("mean", values, None, span),
    median(value: Distribution) => statistic("median", vec![Value::Distribution(value.clone())], span),
    median(values: [Number]) => numeric_statistic("median", values, None, span),
    quantile(value: Distribution, probability: Number) => statistic("quantile", vec![Value::Distribution(value.clone()), Value::Number(probability)], span),
    quantile(values: [Number], probability: Number) => numeric_statistic("quantile", values, Some(probability), span),
    stdev(value: Distribution) => statistic("stdev", vec![Value::Distribution(value.clone())], span),
    stdev(values: [Number]) => numeric_statistic("stdev", values, None, span),
    variance(value: Distribution) => statistic("variance", vec![Value::Distribution(value.clone())], span),
    variance(values: [Number]) => numeric_statistic("variance", values, None, span),
    min(value: Distribution) => statistic("min", vec![Value::Distribution(value.clone())], span),
    min(values: [Number]) => numeric_statistic("min", values, None, span),
    min(values: Array) => saturate(runtime, "min", values.clone(), span),
    max(value: Distribution) => statistic("max", vec![Value::Distribution(value.clone())], span),
    max(values: [Number]) => numeric_statistic("max", values, None, span),
    max(values: Array) => saturate(runtime, "max", values.clone(), span),
    mode(value: Distribution) => statistic("mode", vec![Value::Distribution(value.clone())], span),
    sort(values: [Number]) => numeric_list("sort", vec![numbers(values)], span),
    cumsum(values: [Number]) => numeric_list("cumsum", vec![numbers(values)], span),
    cumprod(values: [Number]) => numeric_list("cumprod", vec![numbers(values)], span),
    diff(values: [Number]) => numeric_list("diff", vec![numbers(values)], span),
    "List.length"(values: Array) => list_query("List.length", vec![Value::Array(values.clone())], span),
    "List.first"(values: Array) => list_query("List.first", vec![Value::Array(values.clone())], span),
    "List.last"(values: Array) => list_query("List.last", vec![Value::Array(values.clone())], span),
    "List.reverse"(values: Array) => list_query("List.reverse", vec![Value::Array(values.clone())], span),
    "List.concat"(left: Array, right: Array) => list_transform("List.concat", vec![Value::Array(left.clone()), Value::Array(right.clone())], span),
    "List.append"(values: Array, value: *) => list_transform("List.append", vec![Value::Array(values.clone()), value.clone()], span),
    "List.slice"(values: Array, start: NonNegativeInteger) => list_transform("List.slice", vec![Value::Array(values.clone()), Value::Number(start as f64)], span),
    "List.slice"(values: Array, start: NonNegativeInteger, end: NonNegativeInteger) => list_transform("List.slice", vec![Value::Array(values.clone()), Value::Number(start as f64), Value::Number(end as f64)], span),
    "List.upTo"(low: Integer, high: Integer) => list_transform("List.upTo", vec![Value::Number(low as f64), Value::Number(high as f64)], span),
    "List.map"(values: Array, function: Function) => higher_order(runtime, "List.map", vec![Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.reduce"(values: Array, initial: *, function: Function) => higher_order(runtime, "List.reduce", vec![Value::Array(values.clone()), initial.clone(), Value::Function(function.clone())], span),
    "List.filter"(values: Array, function: Function) => higher_order(runtime, "List.filter", vec![Value::Array(values.clone()), Value::Function(function.clone())], span),
    "Dict.set"(values: Dictionary, key: String, value: *) => dictionary(runtime, "Dict.set", vec![Value::Dictionary(values.clone()), Value::String(key.clone()), value.clone()], span),
    "Dict.has"(values: Dictionary, key: String) => dictionary(runtime, "Dict.has", vec![Value::Dictionary(values.clone()), Value::String(key.clone())], span),
    "Dict.size"(values: Dictionary) => dictionary(runtime, "Dict.size", vec![Value::Dictionary(values.clone())], span),
    "Dict.delete"(values: Dictionary, key: String) => dictionary(runtime, "Dict.delete", vec![Value::Dictionary(values.clone()), Value::String(key.clone())], span),
    "Dict.merge"(left: Dictionary, right: Dictionary) => dictionary(runtime, "Dict.merge", vec![Value::Dictionary(left.clone()), Value::Dictionary(right.clone())], span),
    "Dict.keys"(values: Dictionary) => dictionary(runtime, "Dict.keys", vec![Value::Dictionary(values.clone())], span),
    "Dict.values"(values: Dictionary) => dictionary(runtime, "Dict.values", vec![Value::Dictionary(values.clone())], span),
    "Dict.map"(values: Dictionary, function: Function) => dictionary(runtime, "Dict.map", vec![Value::Dictionary(values.clone()), Value::Function(function.clone())], span),
}

fn numbers(values: Vec<f64>) -> Value {
    Value::Array(values.into_iter().map(Value::Number).collect())
}

/// Takes an extremum across values at matching draw indices.
///
/// Saturation is the defining operation of a capacity model: throughput is the
/// lesser of offered load and capacity, and headroom is the greater of zero and
/// the difference. Applying the extremum per draw rather than to whole
/// distributions is what makes it meaningful, because the smaller of two
/// distributions is not a property of their summaries. Where demand and capacity
/// overlap, some draws saturate and others do not, and only a per-draw extremum
/// reproduces the resulting mixture and the share of draws that bind.
///
/// Comparison folds with [`f64::total_cmp`] so the result agrees with the
/// numeric path in every case, including signed zero.
fn saturate(
    runtime: &mut Runtime,
    name: &str,
    values: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    if values.is_empty() {
        return Err(Diagnostic::runtime("list must not be empty", span));
    }
    let take_least = name == "min";
    elementwise(runtime, &values, span, move |row| {
        row.iter()
            .copied()
            .reduce(|left, right| {
                let ordering = right.total_cmp(&left);
                if (take_least && ordering.is_lt()) || (!take_least && ordering.is_gt()) {
                    right
                } else {
                    left
                }
            })
            .ok_or_else(|| "list must not be empty".to_owned())
    })
}

/// Computes one summary statistic over an already-extracted list of numbers.
///
/// The general [`statistic`] path exists for arguments that arrive as values and
/// may hold a distribution. Routing a number list through it meant rebuilding the
/// list into an `Array` only to take it apart again, and then sorting it
/// regardless of which statistic was asked for. Projections pay that repeatedly,
/// since a `min`/`max` clamp is the ordinary way to bound a state.
///
/// An extremum is the only case that can skip the sort, and it folds with
/// [`f64::total_cmp`] rather than [`f64::min`] so that it agrees with taking the
/// first or last element of the sorted list in every case, including signed zero,
/// where the primitive comparison treats `-0.0` and `0.0` as equal and a total
/// order does not. The remaining statistics still sort first, because summing in
/// sorted order is part of the result: floating-point addition is not
/// associative, so summing the list as written would round differently.
fn numeric_statistic(
    name: &str,
    values: Vec<f64>,
    probability: Option<f64>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let empty = || Diagnostic::runtime("list must not be empty", span);
    if let "min" | "max" = name {
        let extremum = if name == "min" {
            values
                .into_iter()
                .reduce(|a, b| if b.total_cmp(&a).is_lt() { b } else { a })
        } else {
            values
                .into_iter()
                .reduce(|a, b| if b.total_cmp(&a).is_gt() { b } else { a })
        };
        return Ok(Value::Number(extremum.ok_or_else(empty)?));
    }
    if values.is_empty() {
        return Err(empty());
    }
    let mut sorted = values;
    sorted.sort_by(f64::total_cmp);
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    let result = match name {
        "mean" => mean,
        "stdev" => dispersion(&sorted, mean).sqrt(),
        "variance" => dispersion(&sorted, mean),
        _ => quantile(&sorted, probability.unwrap_or(0.5)),
    };
    Ok(Value::Number(result))
}

fn dispersion(values: &[f64], mean: f64) -> f64 {
    values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64
}

fn fold(
    runtime: &mut Runtime,
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(&arguments, 1, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    let mut result = Value::Number(if name == "sum" { 0.0 } else { 1.0 });
    let operator = if name == "sum" { "+" } else { "*" };
    for value in values {
        result = runtime.binary(operator, result, value.clone(), span)?;
    }
    Ok(result)
}

fn statistic(name: &str, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    let expected_arity = if name == "quantile" { 2 } else { 1 };
    arity(&arguments, expected_arity, span)?;
    if let Value::Distribution(distribution) = &arguments[0] {
        let result = match name {
            "mean" => distribution.mean(),
            "median" => distribution.quantile(0.5),
            "quantile" => distribution.quantile(number(&arguments[1], span)?),
            "stdev" => distribution.stdev(),
            "variance" => distribution.variance(),
            "min" => distribution.minimum(),
            "max" => distribution.maximum(),
            _ => distribution.mode(),
        };
        return result
            .map(Value::Number)
            .map_err(|error| Diagnostic::runtime(error, span));
    }
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array or Distribution", &arguments[0], span));
    };
    let mut numbers = values
        .iter()
        .map(|value| number(value, span))
        .collect::<Result<Vec<_>, _>>()?;
    if numbers.is_empty() && !matches!(name, "sum" | "product") {
        return Err(Diagnostic::runtime("list must not be empty", span));
    }
    numbers.sort_by(f64::total_cmp);
    let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
    let result = match name {
        "mean" => mean,
        "median" => quantile(&numbers, 0.5),
        "quantile" => quantile(&numbers, number(&arguments[1], span)?),
        "stdev" => (numbers
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / numbers.len() as f64)
            .sqrt(),
        "variance" => {
            numbers
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / numbers.len() as f64
        }
        "min" => numbers
            .first()
            .copied()
            .ok_or_else(|| Diagnostic::runtime("list must not be empty", span))?,
        "max" => numbers
            .last()
            .copied()
            .ok_or_else(|| Diagnostic::runtime("list must not be empty", span))?,
        _ => {
            return Err(Diagnostic::runtime(
                "mode is only supported for distributions",
                span,
            ));
        }
    };
    Ok(Value::Number(result))
}

fn numeric_list(name: &str, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    arity(&arguments, 1, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    let mut numbers = values
        .iter()
        .map(|value| number(value, span))
        .collect::<Result<Vec<_>, _>>()?;
    match name {
        "sort" => numbers.sort_by(f64::total_cmp),
        "cumsum" => {
            for index in 1..numbers.len() {
                numbers[index] += numbers[index - 1];
            }
        }
        "cumprod" => {
            for index in 1..numbers.len() {
                numbers[index] *= numbers[index - 1];
            }
        }
        _ => numbers = numbers.windows(2).map(|pair| pair[1] - pair[0]).collect(),
    }
    Ok(Value::Array(
        numbers.into_iter().map(Value::Number).collect(),
    ))
}

fn list_query(name: &str, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    arity(&arguments, 1, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    match name {
        "List.length" => Ok(Value::Number(values.len() as f64)),
        "List.first" => values
            .first()
            .cloned()
            .ok_or_else(|| Diagnostic::runtime("list must not be empty", span)),
        "List.last" => values
            .last()
            .cloned()
            .ok_or_else(|| Diagnostic::runtime("list must not be empty", span)),
        _ => Ok(Value::Array(values.iter().cloned().rev().collect())),
    }
}

fn list_transform(name: &str, arguments: Vec<Value>, span: Span) -> Result<Value, Diagnostic> {
    if name == "List.upTo" {
        arity(&arguments, 2, span)?;
        let low = number(&arguments[0], span)? as i64;
        let high = number(&arguments[1], span)? as i64;
        return Ok(Value::Array(
            (low..=high)
                .map(|value| Value::Number(value as f64))
                .collect(),
        ));
    }
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    match name {
        "List.concat" => {
            arity(&arguments, 2, span)?;
            let Value::Array(right) = &arguments[1] else {
                return Err(expected("Array", &arguments[1], span));
            };
            Ok(Value::Array(values.iter().chain(right).cloned().collect()))
        }
        "List.append" => {
            arity(&arguments, 2, span)?;
            let mut result = values.clone();
            result.push(arguments[1].clone());
            Ok(Value::Array(result))
        }
        _ => {
            if !(2..=3).contains(&arguments.len()) {
                return Err(Diagnostic::runtime(
                    "List.slice expects 2 or 3 arguments",
                    span,
                ));
            }
            let start = number(&arguments[1], span)? as usize;
            let end = arguments
                .get(2)
                .map(|value| number(value, span))
                .transpose()?
                .map_or(values.len(), |value| value as usize);
            Ok(Value::Array(values.get(start..end).unwrap_or(&[]).to_vec()))
        }
    }
}

fn higher_order(
    runtime: &mut Runtime,
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let expected_arity = if name == "List.reduce" { 3 } else { 2 };
    arity(&arguments, expected_arity, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    let function_index = if name == "List.reduce" { 2 } else { 1 };
    if !matches!(arguments[function_index], Value::Function(_)) {
        return Err(expected("Function", &arguments[function_index], span));
    }
    if name == "List.reduce" {
        let mut result = arguments[1].clone();
        for value in values {
            result = runtime.call(arguments[2].clone(), vec![result, value.clone()], span)?;
        }
        return Ok(result);
    }
    let mut result = Vec::new();
    for (index, value) in values.iter().enumerate() {
        let function = arguments[1].clone();
        let arity = if let Value::Function(function) = &function {
            function.arity()
        } else {
            None
        };
        let args = if arity == Some(2) {
            vec![value.clone(), Value::Number(index as f64)]
        } else {
            vec![value.clone()]
        };
        let mapped = runtime.call(function, args, span)?;
        if name == "List.map" {
            result.push(mapped);
        } else if mapped == Value::Boolean(true) {
            result.push(value.clone());
        } else if mapped != Value::Boolean(false) {
            return Err(expected("Boolean callback result", &mapped, span));
        }
    }
    Ok(Value::Array(result))
}

fn dictionary(
    runtime: &mut Runtime,
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    let Value::Dictionary(values) = &arguments[0] else {
        return Err(expected("Dictionary", &arguments[0], span));
    };
    match name {
        "Dict.size" => {
            arity(&arguments, 1, span)?;
            Ok(Value::Number(values.len() as f64))
        }
        "Dict.has" => {
            arity(&arguments, 2, span)?;
            Ok(Value::Boolean(
                values.contains_key(&string(&arguments[1], span)?),
            ))
        }
        "Dict.keys" => {
            arity(&arguments, 1, span)?;
            Ok(Value::Array(
                values.keys().cloned().map(Value::String).collect(),
            ))
        }
        "Dict.values" => {
            arity(&arguments, 1, span)?;
            Ok(Value::Array(values.values().cloned().collect()))
        }
        "Dict.set" => {
            arity(&arguments, 3, span)?;
            let mut result = values.as_ref().clone();
            result.insert(string(&arguments[1], span)?, arguments[2].clone());
            Ok(Value::dictionary(result))
        }
        "Dict.delete" => {
            arity(&arguments, 2, span)?;
            let mut result = values.as_ref().clone();
            result.remove(&string(&arguments[1], span)?);
            Ok(Value::dictionary(result))
        }
        "Dict.merge" => {
            arity(&arguments, 2, span)?;
            let Value::Dictionary(right) = &arguments[1] else {
                return Err(expected("Dictionary", &arguments[1], span));
            };
            let mut result = values.as_ref().clone();
            result.extend(right.as_ref().clone());
            Ok(Value::dictionary(result))
        }
        _ => {
            arity(&arguments, 2, span)?;
            let mut result = BTreeMap::new();
            for (key, value) in values.iter() {
                result.insert(
                    key.clone(),
                    runtime.call(arguments[1].clone(), vec![value.clone()], span)?,
                );
            }
            Ok(Value::dictionary(result))
        }
    }
}

fn quantile(sorted: &[f64], probability: f64) -> f64 {
    let position = probability.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let low = position.floor() as usize;
    let high = position.ceil() as usize;
    sorted[low] + (sorted[high] - sorted[low]) * position.fract()
}
fn string(value: &Value, span: Span) -> Result<String, Diagnostic> {
    if let Value::String(value) = value {
        Ok(value.clone())
    } else {
        Err(expected("String", value, span))
    }
}
fn expected(expected: &str, value: &Value, span: Span) -> Diagnostic {
    Diagnostic::runtime(
        format!("expected {expected}, received {}", value.type_name()),
        span,
    )
}

#[cfg(test)]
mod tests {
    use super::super::Runtime;

    fn evaluate(source: &str) -> f64 {
        Runtime::new()
            .evaluate(source)
            .unwrap()
            .as_number()
            .expect("a number")
    }

    /// The fast path for extrema must order floats exactly as sorting did.
    ///
    /// `f64::min` and `f64::max` treat `-0.0` and `0.0` as equal and may return
    /// either, while taking the first or last element of a list sorted by
    /// `total_cmp` always distinguishes them. Skipping the sort is only sound if
    /// the fold reproduces that order.
    #[test]
    fn distinguishes_signed_zero_the_way_a_total_order_does() {
        assert!(evaluate("min([0, -0])").is_sign_negative());
        assert!(evaluate("max([-0, 0])").is_sign_positive());
    }

    #[test]
    fn summarises_a_list_without_reordering_the_caller_visible_result() {
        assert_eq!(evaluate("min([3, 1, 2])"), 1.0);
        assert_eq!(evaluate("max([3, 1, 2])"), 3.0);
        assert_eq!(evaluate("mean([1, 2, 6])"), 3.0);
        assert_eq!(evaluate("median([3, 1, 2])"), 2.0);
        assert_eq!(evaluate("variance([1, 2, 3])"), 2.0 / 3.0);
    }
}
