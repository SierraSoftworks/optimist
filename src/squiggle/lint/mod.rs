//! Static diagnostics for Squiggle calculations.
//!
//! The linter is conservative: it reports contradictions that can be proven from
//! source types and builtin signatures while allowing values whose type depends on
//! imports, dynamic callbacks, or heterogeneous collections.

mod call;
mod checker;
mod expression;
mod metadata;
mod types;

pub(crate) use metadata::{BuiltinSignature, Constraint, ParameterConstraint};

use super::{Diagnostic, parse};

/// Parses and statically checks a Squiggle calculation without evaluating it.
///
/// Syntax errors, unknown identifiers, invalid calls, incompatible operators,
/// invalid lookups, non-Boolean conditions, and incompatible `::` unit signatures
/// are returned as source-spanned diagnostics.
pub fn lint(source: &str) -> Vec<Diagnostic> {
    match parse(source) {
        Ok(program) => checker::Checker::new().check(&program),
        Err(diagnostics) => diagnostics,
    }
}
