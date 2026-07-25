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

use super::{Diagnostic, ast::Program, parse};

/// Parses and statically checks a Squiggle calculation without evaluating it.
///
/// Syntax errors, unknown identifiers, invalid calls, incompatible operators,
/// invalid lookups, non-Boolean conditions, and incompatible `::` unit signatures
/// are returned as source-spanned diagnostics.
pub fn lint(source: &str) -> Vec<Diagnostic> {
    match parse(source) {
        Ok(program) => lint_program(&program),
        Err(diagnostics) => diagnostics,
    }
}

/// Statically checks an already-parsed calculation.
///
/// Callers that also evaluate a program use this to parse once rather than
/// separately for checking and for running, which halves the parsing work on the
/// request path. The parser is rebuilt on every [`parse`] call, so the saving is
/// proportional to source size rather than negligible.
pub fn lint_program(program: &Program) -> Vec<Diagnostic> {
    checker::Checker::new().check(program)
}
