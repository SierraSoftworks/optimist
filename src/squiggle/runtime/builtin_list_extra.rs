use rand::seq::SliceRandom;

use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::{
    Runtime,
    builtin::{arity, number},
};

builtins! {
    context(runtime, span);
    "List.make"(count: NonNegativeInteger, function: Function) => make(runtime, &[Value::Number(count as f64), Value::Function(function.clone())], span),
    "List.make"(count: NonNegativeInteger, value: *) => make(runtime, &[Value::Number(count as f64), value.clone()], span),
    "List.flatten"(values: Array) => flatten(&[Value::Array(values.clone())], span),
    "List.join"(values: Array, separator: String) => join(&[Value::Array(values.clone()), Value::String(separator.clone())], span),
    "List.zip"(first: Array, ...rest: Array) => {
        let mut arrays = Vec::with_capacity(rest.len() + 1);
        arrays.push(Value::Array(first.clone()));
        arrays.extend(rest.into_iter().map(|values| Value::Array(values.clone())));
        zip(&arrays, span)
    },
    "List.unzip"(rows: [Array]) => unzip(&[Value::Array(rows.into_iter().map(|row| Value::Array(row.clone())).collect())], span),
    "List.uniq"(values: Array) => unique(&[Value::Array(values.clone())], span),
    "List.shuffle"(values: Array) => random(runtime, "List.shuffle", &[Value::Array(values.clone())], span),
    "List.sample"(values: Array) => random(runtime, "List.sample", &[Value::Array(values.clone())], span),
    "List.sampleN"(values: Array, count: NonNegativeInteger) => random(runtime, "List.sampleN", &[Value::Array(values.clone()), Value::Number(count as f64)], span),
    "List.reduceReverse"(values: Array, initial: *, function: Function) => reduce(runtime, "List.reduceReverse", &[Value::Array(values.clone()), initial.clone(), Value::Function(function.clone())], span),
    "List.reduceWhile"(values: Array, initial: *, step: Function, condition: Function) => reduce(runtime, "List.reduceWhile", &[Value::Array(values.clone()), initial.clone(), Value::Function(step.clone()), Value::Function(condition.clone())], span),
    "List.every"(values: Array, function: Function) => predicate_or_order(runtime, "List.every", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.some"(values: Array, function: Function) => predicate_or_order(runtime, "List.some", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.find"(values: Array, function: Function) => predicate_or_order(runtime, "List.find", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.findIndex"(values: Array, function: Function) => predicate_or_order(runtime, "List.findIndex", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.sortBy"(values: Array, function: Function) => predicate_or_order(runtime, "List.sortBy", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.minBy"(values: Array, function: Function) => predicate_or_order(runtime, "List.minBy", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
    "List.maxBy"(values: Array, function: Function) => predicate_or_order(runtime, "List.maxBy", &[Value::Array(values.clone()), Value::Function(function.clone())], span),
}

fn make(runtime: &mut Runtime, arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 2, span)?;
    let count = integer(&arguments[0], span)?;
    if let Value::Function(function) = &arguments[1] {
        let use_index = function.arity() == Some(1);
        return (0..count)
            .map(|index| {
                let indexed = [Value::Number(index as f64)];
                let args: &[Value] = if use_index { &indexed } else { &[] };
                runtime.call(arguments[1].clone(), args, span)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }
    Ok(Value::Array(vec![arguments[1].clone(); count]))
}

fn flatten(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    let values = array(arguments, 1, span)?;
    let mut result = Vec::new();
    for value in values {
        if let Value::Array(nested) = value {
            result.extend(nested.clone());
        } else {
            result.push(value.clone());
        }
    }
    Ok(Value::Array(result))
}

fn join(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    arity(arguments, 2, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    let Value::String(separator) = &arguments[1] else {
        return Err(expected("String", &arguments[1], span));
    };
    Ok(Value::String(
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(separator),
    ))
}

fn zip(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    if arguments.is_empty() {
        return Err(Diagnostic::runtime(
            "List.zip requires at least one array",
            span,
        ));
    }
    let arrays = arguments
        .iter()
        .map(|value| {
            if let Value::Array(values) = value {
                Ok(values)
            } else {
                Err(expected("Array", value, span))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = arrays
        .iter()
        .map(|values| values.len())
        .min()
        .ok_or_else(|| Diagnostic::runtime("List.zip requires at least one array", span))?;
    Ok(Value::Array(
        (0..count)
            .map(|index| Value::Array(arrays.iter().map(|values| values[index].clone()).collect()))
            .collect(),
    ))
}

fn unzip(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    let rows = array(arguments, 1, span)?;
    let width = rows
        .first()
        .and_then(|row| {
            if let Value::Array(values) = row {
                Some(values.len())
            } else {
                None
            }
        })
        .unwrap_or(0);
    let mut result = vec![Vec::new(); width];
    for row in rows {
        let Value::Array(values) = row else {
            return Err(expected("nested Array", row, span));
        };
        if values.len() != width {
            return Err(Diagnostic::runtime(
                "List.unzip rows must have equal lengths",
                span,
            ));
        }
        for (index, value) in values.iter().enumerate() {
            result[index].push(value.clone());
        }
    }
    Ok(Value::Array(result.into_iter().map(Value::Array).collect()))
}

fn unique(arguments: &[Value], span: Span) -> Result<Value, Diagnostic> {
    let values = array(arguments, 1, span)?;
    let mut result = Vec::new();
    for value in values {
        if !result.contains(value) {
            result.push(value.clone());
        }
    }
    Ok(Value::Array(result))
}

fn random(
    runtime: &mut Runtime,
    name: &str,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    let expected_arity = if name == "List.sampleN" { 2 } else { 1 };
    arity(arguments, expected_arity, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    if values.is_empty() {
        return Err(Diagnostic::runtime("list must not be empty", span));
    }
    if name == "List.shuffle" {
        let mut result = values.clone();
        result.shuffle(&mut runtime.rng);
        return Ok(Value::Array(result));
    }
    if name == "List.sample" {
        return values
            .choose(&mut runtime.rng)
            .cloned()
            .ok_or_else(|| Diagnostic::runtime("list must not be empty", span));
    }
    let count = integer(&arguments[1], span)?;
    let samples = (0..count)
        .map(|_| {
            values
                .choose(&mut runtime.rng)
                .cloned()
                .ok_or_else(|| Diagnostic::runtime("list must not be empty", span))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::Array(samples))
}

fn reduce(
    runtime: &mut Runtime,
    name: &str,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    let expected_arity = if name == "List.reduceWhile" { 4 } else { 3 };
    arity(arguments, expected_arity, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    let mut result = arguments[1].clone();
    let iterator: Box<dyn Iterator<Item = &Value>> = if name == "List.reduceReverse" {
        Box::new(values.iter().rev())
    } else {
        Box::new(values.iter())
    };
    for value in iterator {
        let next = runtime.call(arguments[2].clone(), &[result.clone(), value.clone()], span)?;
        if name == "List.reduceWhile" {
            match runtime.call(arguments[3].clone(), std::slice::from_ref(&next), span)? {
                Value::Boolean(true) => {}
                Value::Boolean(false) => break,
                value => return Err(expected("Boolean", &value, span)),
            }
        }
        result = next;
    }
    Ok(result)
}

fn predicate_or_order(
    runtime: &mut Runtime,
    name: &str,
    arguments: &[Value],
    span: Span,
) -> Result<Value, Diagnostic> {
    arity(arguments, 2, span)?;
    let Value::Array(values) = &arguments[0] else {
        return Err(expected("Array", &arguments[0], span));
    };
    if matches!(name, "List.sortBy" | "List.minBy" | "List.maxBy") {
        let mut keyed = values
            .iter()
            .map(|value| {
                Ok((
                    number(
                        &runtime.call(arguments[1].clone(), std::slice::from_ref(value), span)?,
                        span,
                    )?,
                    value.clone(),
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        keyed.sort_by(|left, right| left.0.total_cmp(&right.0));
        return if name == "List.sortBy" {
            Ok(Value::Array(keyed.into_iter().map(|pair| pair.1).collect()))
        } else if name == "List.minBy" {
            keyed
                .first()
                .map(|pair| pair.1.clone())
                .ok_or_else(|| Diagnostic::runtime("list must not be empty", span))
        } else {
            keyed
                .last()
                .map(|pair| pair.1.clone())
                .ok_or_else(|| Diagnostic::runtime("list must not be empty", span))
        };
    }
    for (index, value) in values.iter().enumerate() {
        let result = runtime.call(arguments[1].clone(), std::slice::from_ref(value), span)?;
        let Value::Boolean(matches) = result else {
            return Err(expected("Boolean", &result, span));
        };
        if matches && name == "List.some" {
            return Ok(Value::Boolean(true));
        }
        if !matches && name == "List.every" {
            return Ok(Value::Boolean(false));
        }
        if matches && name == "List.find" {
            return Ok(value.clone());
        }
        if matches && name == "List.findIndex" {
            return Ok(Value::Number(index as f64));
        }
    }
    Ok(match name {
        "List.some" => Value::Boolean(false),
        "List.every" => Value::Boolean(true),
        "List.findIndex" => Value::Number(-1.0),
        _ => Value::Void,
    })
}

fn array(arguments: &[Value], expected_arity: usize, span: Span) -> Result<&[Value], Diagnostic> {
    arity(arguments, expected_arity, span)?;
    if let Value::Array(values) = &arguments[0] {
        Ok(values)
    } else {
        Err(expected("Array", &arguments[0], span))
    }
}
fn integer(value: &Value, span: Span) -> Result<usize, Diagnostic> {
    let value = number(value, span)?;
    if value >= 0.0 && value.fract() == 0.0 {
        Ok(value as usize)
    } else {
        Err(Diagnostic::runtime("expected a non-negative integer", span))
    }
}
fn expected(expected: &str, value: &Value, span: Span) -> Diagnostic {
    Diagnostic::runtime(
        format!("expected {expected}, received {}", value.type_name()),
        span,
    )
}
