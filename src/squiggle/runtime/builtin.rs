use crate::squiggle::{Diagnostic, Value, ast::Span};

use super::{
    Runtime, builtin_collection, builtin_common, builtin_core, builtin_dict_extra,
    builtin_distribution, builtin_domain, builtin_list_extra, builtin_queueing,
    builtin_reliability, builtin_sampleset, builtin_scoring, builtin_temporal,
};

pub(crate) fn signatures() -> Vec<crate::squiggle::lint::BuiltinSignature> {
    let mut signatures = Vec::new();
    signatures.extend(builtin_common::signatures());
    signatures.extend(builtin_core::signatures());
    signatures.extend(builtin_list_extra::signatures());
    signatures.extend(builtin_dict_extra::signatures());
    signatures.extend(builtin_domain::signatures());
    signatures.extend(builtin_temporal::signatures());
    signatures.extend(builtin_scoring::signatures());
    signatures.extend(builtin_sampleset::signatures());
    signatures.extend(builtin_distribution::signatures());
    signatures.extend(builtin_queueing::signatures());
    signatures.extend(builtin_reliability::signatures());
    signatures.extend(builtin_collection::signatures());
    signatures
}

pub(super) fn call(
    runtime: &mut Runtime,
    name: &str,
    arguments: Vec<Value>,
    span: Span,
) -> Result<Value, Diagnostic> {
    if builtin_common::handles(name) {
        return builtin_common::call(runtime, name, arguments, span);
    }
    if builtin_core::handles(name) {
        return builtin_core::call(runtime, name, arguments, span);
    }
    if builtin_list_extra::handles(name) {
        return builtin_list_extra::call(runtime, name, arguments, span);
    }
    if builtin_dict_extra::handles(name) {
        return builtin_dict_extra::call(runtime, name, arguments, span);
    }
    if builtin_domain::handles(name) {
        return builtin_domain::call(runtime, name, arguments, span);
    }
    if builtin_temporal::handles(name) {
        return builtin_temporal::call(runtime, name, arguments, span);
    }
    if builtin_scoring::handles(name) {
        return builtin_scoring::call(runtime, name, arguments, span);
    }
    if builtin_sampleset::handles(name) {
        return builtin_sampleset::call(runtime, name, arguments, span);
    }
    if builtin_distribution::handles(name) {
        return builtin_distribution::call(runtime, name, arguments, span);
    }
    if builtin_queueing::handles(name) {
        return builtin_queueing::call(runtime, name, arguments, span);
    }
    if builtin_reliability::handles(name) {
        return builtin_reliability::call(runtime, name, arguments, span);
    }
    if builtin_collection::handles(name) {
        return builtin_collection::call(runtime, name, arguments, span);
    }
    Err(Diagnostic::runtime(
        format!("builtin '{name}' is not implemented"),
        span,
    ))
}

pub(super) fn arity(arguments: &[Value], expected: usize, span: Span) -> Result<(), Diagnostic> {
    (arguments.len() == expected).then_some(()).ok_or_else(|| {
        Diagnostic::runtime(
            format!(
                "expected {expected} arguments, received {}",
                arguments.len()
            ),
            span,
        )
    })
}

pub(super) fn number(value: &Value, span: Span) -> Result<f64, Diagnostic> {
    value.as_number().ok_or_else(|| {
        Diagnostic::runtime(
            format!("expected Number, received {}", value.type_name()),
            span,
        )
    })
}
