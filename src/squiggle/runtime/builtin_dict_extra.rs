use std::{collections::BTreeMap, rc::Rc};

use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::Runtime;

builtins! {
    context(runtime, span);
    "Dict.fromList"(entries: [Array]) => from_list(entries, span),
    "Dict.toList"(values: Dictionary) => Ok(to_list(values)),
    "Dict.mergeMany"(values: [Dictionary]) => Ok(merge_many(values)),
    "Dict.mapKeys"(values: Dictionary, function: Function) => {
        map_keys(runtime, values, function, span)
    },
    "Dict.pick"(values: Dictionary, keys: [String]) => Ok(select(values, keys, true)),
    "Dict.omit"(values: Dictionary, keys: [String]) => Ok(select(values, keys, false)),
}

fn from_list(entries: Vec<&Vec<Value>>, span: Span) -> Result<Value, Diagnostic> {
    let mut result = BTreeMap::new();
    for entry in entries {
        let [Value::String(key), value] = entry.as_slice() else {
            return Err(Diagnostic::runtime(
                "dictionary entries must be [String, value] pairs",
                span,
            ));
        };
        result.insert(key.clone(), value.clone());
    }
    Ok(Value::dictionary(result))
}

fn to_list(values: &BTreeMap<String, Value>) -> Value {
    Value::Array(
        values
            .iter()
            .map(|(key, value)| Value::Array(vec![Value::String(key.clone()), value.clone()]))
            .collect(),
    )
}

fn merge_many(dicts: Vec<&Rc<BTreeMap<String, Value>>>) -> Value {
    let mut result = BTreeMap::new();
    for values in dicts {
        result.extend(values.as_ref().clone());
    }
    Value::dictionary(result)
}

fn map_keys(
    runtime: &mut Runtime,
    values: &BTreeMap<String, Value>,
    function: &crate::squiggle::Function,
    span: Span,
) -> Result<Value, Diagnostic> {
    let mut result = BTreeMap::new();
    for (key, value) in values {
        let mapped = runtime.call(
            Value::Function(function.clone()),
            &[Value::String(key.clone())],
            span,
        )?;
        let Value::String(mapped) = mapped else {
            return Err(Diagnostic::runtime(
                "Dict.mapKeys callback must return String",
                span,
            ));
        };
        result.insert(mapped, value.clone());
    }
    Ok(Value::dictionary(result))
}

fn select(values: &BTreeMap<String, Value>, keys: Vec<&String>, include: bool) -> Value {
    Value::dictionary(
        values
            .iter()
            .filter(|(key, _)| keys.contains(key) == include)
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}
