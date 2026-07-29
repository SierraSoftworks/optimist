use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::Runtime;

builtins! {
    context(runtime, span);
    typeOf(value: *) => Ok(Value::String(value.type_name().into())),
    inspect(value: *) => Ok(value.clone()),
    inspect(value: *, _message: String) => Ok(value.clone()),
    throw() => Err(Diagnostic::runtime("Common.throw() was called", span)),
    throw(message: *) => Err(Diagnostic::runtime(ToString::to_string(message), span)),
    try(function: Function, fallback: Function) => {
        match runtime.call(Value::Function(function.clone()), Vec::new(), span) {
            Ok(value) => tagged("1", value),
            Err(_) => tagged(
                "2",
                runtime.call(Value::Function(fallback.clone()), Vec::new(), span)?,
            ),
        }
    },
    "String.make"(value: *) => Ok(Value::String(value.to_string())),
    "String.make"(value: Number, format: String) => {
        let precision = format
            .trim_start_matches('.')
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(2);
        Ok(Value::String(format!("{value:.precision$}")))
    },
    "String.split"(value: String, separator: String) => Ok(Value::Array(
        value
            .split(separator)
            .map(|part| Value::String(part.into()))
            .collect(),
    )),
    "System.sampleCount"() => Ok(Value::Number(runtime.config.sample_count as f64)),
}

fn tagged(tag: &str, value: Value) -> Result<Value, Diagnostic> {
    Ok(Value::dictionary(
        [
            ("tag".into(), Value::String(tag.into())),
            ("value".into(), value),
        ]
        .into(),
    ))
}
