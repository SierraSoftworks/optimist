use axum::http::StatusCode;

use crate::project::FormulaCommandError;

pub(super) fn classify(
    error: &FormulaCommandError,
) -> (StatusCode, &'static str, &'static [&'static str]) {
    match error {
        FormulaCommandError::CrossProjectAddress(_)
        | FormulaCommandError::RootAddress(_)
        | FormulaCommandError::MissingPrimitiveRoot(_)
        | FormulaCommandError::MissingParent(_)
        | FormulaCommandError::InvalidPrimitiveUnit(_)
        | FormulaCommandError::Formula(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_formula",
            &[
                "Check the component address, primitive root, parent, references, arity, bounds, and units.",
            ],
        ),
        FormulaCommandError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "formula_not_found",
            &["List project formulas and retry with a returned component address."],
        ),
        FormulaCommandError::RevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "formula_revision_conflict",
            &["Show the formula document and retry with its current revision."],
        ),
        FormulaCommandError::Referenced { .. } | FormulaCommandError::HasDescendant { .. } => (
            StatusCode::CONFLICT,
            "formula_in_use",
            &["Remove dependent or descendant formulas before removing this component."],
        ),
        FormulaCommandError::DuplicatePrimitive(_)
        | FormulaCommandError::RevisionSpaceExhausted => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
    }
}
