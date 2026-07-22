use crate::squiggle::{Diagnostic, Domain, Value, ast::Span};

use super::Runtime;

builtins! {
    context(runtime, span);
    "Number.rangeDomain"(minimum: Number, maximum: Number) => {
        domain(Domain::NumberRange { minimum, maximum }, minimum <= maximum, span)
    },
    "Date.rangeDomain"(minimum: Date, maximum: Date) => {
        domain(Domain::DateRange { minimum, maximum }, minimum <= maximum, span)
    },
}

fn domain(value: Domain, ordered: bool, span: Span) -> Result<Value, Diagnostic> {
    if !ordered {
        return Err(Diagnostic::runtime(
            "domain minimum must not exceed maximum",
            span,
        ));
    }
    Ok(Value::Domain(value))
}
