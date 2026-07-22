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
        EstimateCommandError::NestedAddress(_) => (
            StatusCode::BAD_REQUEST,
            "nested_estimate_address",
            &[
                "Use a root estimate address without `/component/...` for primitive estimate commands.",
            ],
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
        EstimateCommandError::ReferencedByFormula { .. } => (
            StatusCode::CONFLICT,
            "estimate_in_use",
            &["Remove formulas rooted under or referencing this estimate before removing it."],
        ),
        EstimateCommandError::Estimate(_) | EstimateCommandError::Quantity(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_estimate_distribution",
            &["Use a distribution whose complete support fits the selected slot's dimension."],
        ),
        EstimateCommandError::Fermi(_)
        | EstimateCommandError::FermiAssessment(_)
        | EstimateCommandError::UnavailableFermiRecommendation => (
            StatusCode::BAD_REQUEST,
            "invalid_fermi_estimate",
            &[
                "Check the equation variables, canonical formula, target unit, sampling controls, and recommendation diagnostics.",
            ],
        ),
        EstimateCommandError::RevisionSpaceExhausted(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
    }
}
