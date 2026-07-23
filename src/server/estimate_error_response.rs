use axum::http::StatusCode;

use crate::project::EstimateCommandError;

pub(super) fn classify(
    error: &EstimateCommandError,
) -> (StatusCode, &'static str, &'static [&'static str]) {
    match error {
        EstimateCommandError::CrossProjectAddress(_) => (
            StatusCode::BAD_REQUEST,
            "cross_project_estimate_address",
            &["Use an estimate address whose project ID matches the selected project."],
        ),
        EstimateCommandError::InvalidSlot { .. } | EstimateCommandError::Slot(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_estimate_slot",
            &["Choose a slot supported by the addressed node or edge payload."],
        ),
        EstimateCommandError::IdentifierConflict(_) | EstimateCommandError::SlotOccupied { .. } => {
            (
                StatusCode::CONFLICT,
                "estimate_conflict",
                &[
                    "Show the owner aggregate and choose an unused estimate ID or the slot's current address.",
                ],
            )
        }
        EstimateCommandError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "estimate_not_found",
            &["Check the address against estimates embedded in the current node or edge payload."],
        ),
        EstimateCommandError::Required { .. } => (
            StatusCode::CONFLICT,
            "required_estimate",
            &[
                "Required causal effect, destination response, and blocking degree estimates may be replaced but not removed.",
            ],
        ),
        EstimateCommandError::ReferencedByDependence(_) => (
            StatusCode::CONFLICT,
            "estimate_in_use",
            &["Replace or remove the project dependence document before removing this estimate."],
        ),
        EstimateCommandError::IncompatibleSupport { .. } => (
            StatusCode::BAD_REQUEST,
            "incompatible_estimate_support",
            &[
                "Use a distribution family with matching support, explicitly truncate the Squiggle result, or edit the entity state type.",
            ],
        ),
        EstimateCommandError::Estimate(_) | EstimateCommandError::Quantity(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_estimate_distribution",
            &["Use a distribution whose complete support fits the selected slot's dimension."],
        ),
        EstimateCommandError::Squiggle(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_squiggle_estimate",
            &[
                "Check Squiggle syntax, result type, unit annotations, support, and deterministic evaluation controls.",
            ],
        ),
        EstimateCommandError::RevisionSpaceExhausted(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
    }
}
