use axum::http::StatusCode;

use crate::project::{AggregateUpdateError, CommandBatchError, EvidenceCommandError, ProjectError};

use super::{estimate_error_response, repository_error_response};

pub(super) fn classify(
    error: &ProjectError,
) -> (StatusCode, &'static str, &'static [&'static str]) {
    match error {
        ProjectError::EmptyName => (
            StatusCode::BAD_REQUEST,
            "invalid_project_name",
            &["Provide a non-empty project name."],
        ),
        ProjectError::DuplicateName(_) => (
            StatusCode::CONFLICT,
            "project_name_conflict",
            &["Choose a project name which is not already in use."],
        ),
        ProjectError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            "project_not_found",
            &["List projects and retry with one of the returned project IDs."],
        ),
        ProjectError::RevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "project_revision_conflict",
            &["Refresh the project and rebuild the command against its current revision."],
        ),
        ProjectError::CommandBatch(CommandBatchError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            "command_batch_not_found",
            &["Use the ID of a retained, committed forward command batch."],
        ),
        ProjectError::CommandBatch(
            CommandBatchError::RequestConflict(_) | CommandBatchError::AlreadyCompensated { .. },
        ) => (
            StatusCode::CONFLICT,
            "command_batch_conflict",
            &[
                "Use a fresh batch request ID, or inspect replay history before preparing compensation.",
            ],
        ),
        ProjectError::CommandBatch(
            CommandBatchError::Empty
            | CommandBatchError::TooLarge { .. }
            | CommandBatchError::CompensationTarget(_),
        ) => (
            StatusCode::BAD_REQUEST,
            "invalid_command_batch",
            &["Submit between 1 and 100 commands and compensate only a retained forward batch."],
        ),
        ProjectError::CommandBatch(CommandBatchError::RevisionSpaceExhausted) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Create a replacement project because its revision space is exhausted."],
        ),
        ProjectError::InvalidReplayRevision { .. } => (
            StatusCode::BAD_REQUEST,
            "invalid_replay_revision",
            &["Use a replay revision between zero and the project's current revision."],
        ),
        ProjectError::ChangeHistoryGap { .. } => (
            StatusCode::CONFLICT,
            "change_history_gap",
            &[
                "Fetch a current project snapshot, replace local state, and reconnect from the reported available revision.",
            ],
        ),
        ProjectError::Node(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_node",
            &["Provide a non-empty node name and title with fields valid for its node kind."],
        ),
        ProjectError::EdgeId(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_edge_id",
            &["Use an edge ID returned by `optimist edge list`, such as `A-requires-B`."],
        ),
        ProjectError::EstimateAddress(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_estimate_address",
            &[
                "Use `<project>/<node|edge>/<owner>/estimate/<id>` with canonical project, owner, and estimate IDs.",
            ],
        ),
        ProjectError::NotMeasurementEdge(_) => (
            StatusCode::BAD_REQUEST,
            "not_measurement_edge",
            &["Choose a `measures` edge returned by `optimist edge list`."],
        ),
        ProjectError::NotInterventionEffectEdge(_) => (
            StatusCode::BAD_REQUEST,
            "not_intervention_effect_edge",
            &["Choose a `changes` edge, which is the only relationship with a temporal profile."],
        ),
        ProjectError::NotCausalEdge(_) => (
            StatusCode::BAD_REQUEST,
            "not_causal_edge",
            &[
                "Choose a `contributes` or `changes` edge, which are the relationships that own a response.",
            ],
        ),
        ProjectError::CausalResponse(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_causal_response",
            &["Anchor the response to a finite, nonzero source change."],
        ),
        ProjectError::OngoingEffectCannotBeTransient(_) => (
            StatusCode::BAD_REQUEST,
            "ongoing_effect_cannot_be_transient",
            &[
                "Remove the profile and rebound from this `contributes` edge, and shape the intervention's `changes` edge instead.",
            ],
        ),
        ProjectError::EffectProfile(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_effect_profile",
            &[
                "Declare a hold window before configuring a release form, and pair every aftereffect with a rebound magnitude.",
            ],
        ),
        ProjectError::ObservationUnitMismatch { .. } => (
            StatusCode::BAD_REQUEST,
            "observation_unit_mismatch",
            &["Use the unit defined by the measurement edge's source metric."],
        ),
        ProjectError::NativeStateUnsupported(_) => (
            StatusCode::BAD_REQUEST,
            "native_state_unsupported",
            &["Choose a factor or outcome node before configuring native state."],
        ),
        ProjectError::StateQuantityUsedByCausalEdge(_) => (
            StatusCode::CONFLICT,
            "state_quantity_in_use",
            &[
                "Remove or replace incident causal relationships before changing the quantity's canonical unit terms.",
            ],
        ),
        ProjectError::Quantity(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_quantity_state",
            &[
                "Check the canonical unit terms, support bounds, and operational definition, then retry.",
            ],
        ),
        ProjectError::CausalResponseUnitMismatch { .. } => (
            StatusCode::BAD_REQUEST,
            "causal_response_unit_mismatch",
            &[
                "Use source and destination units matching the canonical quantity dimensions on both endpoint nodes.",
            ],
        ),
        ProjectError::MissingQuantityDimension(_) => (
            StatusCode::BAD_REQUEST,
            "missing_quantity_dimension",
            &[
                "Upgrade the metric with canonical unit terms before using it in a causal relationship.",
            ],
        ),
        ProjectError::Observation(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_observation",
            &[
                "Check the value, RFC 3339 timestamp, source, unit, and measurement standard deviation.",
            ],
        ),
        ProjectError::MeasurementCalibration(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_measurement_calibration",
            &[
                "Use finite distinct anchors whose direction matches polarity, or four ordered anchors for a target range.",
            ],
        ),
        ProjectError::EvidenceCommand(EvidenceCommandError::RevisionConflict { .. }) => (
            StatusCode::CONFLICT,
            "evidence_revision_conflict",
            &["Refresh the node and retry with the evidence record's current revision."],
        ),
        ProjectError::EvidenceCommand(EvidenceCommandError::NotFound { .. }) => (
            StatusCode::NOT_FOUND,
            "evidence_not_found",
            &["Refresh the node and choose an evidence record which still exists."],
        ),
        ProjectError::EvidenceCommand(
            EvidenceCommandError::InvalidOwner(_) | EvidenceCommandError::EmptySummary,
        ) => (
            StatusCode::BAD_REQUEST,
            "invalid_evidence",
            &["Choose a factor or outcome and provide a non-empty evidence summary."],
        ),
        ProjectError::AggregateUpdate(AggregateUpdateError::NodeRevisionConflict { .. }) => (
            StatusCode::CONFLICT,
            "node_revision_conflict",
            &["Show the node and retry with its current node and project revisions."],
        ),
        ProjectError::AggregateUpdate(AggregateUpdateError::EdgeRevisionConflict { .. }) => (
            StatusCode::CONFLICT,
            "edge_revision_conflict",
            &["Show the edge and retry with its current edge and project revisions."],
        ),
        ProjectError::AggregateUpdate(AggregateUpdateError::NodeRevisionSpaceExhausted(_)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
        ProjectError::Scenario(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_scenario",
            &[
                "Check the name, title, horizon, unique references, positive weights and budgets, and Monte Carlo configuration.",
            ],
        ),
        ProjectError::ScenarioNotFound(_) => (
            StatusCode::NOT_FOUND,
            "scenario_not_found",
            &["List scenarios and retry with one of the returned scenario IDs."],
        ),
        ProjectError::ScenarioRevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "scenario_revision_conflict",
            &["Show the scenario and retry with its current scenario and project revisions."],
        ),
        ProjectError::InvalidScenarioReference { .. } => (
            StatusCode::BAD_REQUEST,
            "invalid_scenario_reference",
            &["Use outcome IDs for objectives and intervention IDs for candidate interventions."],
        ),
        ProjectError::Dependence(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_dependence",
            &[
                "Check unique same-project members and provide a finite symmetric positive-semidefinite correlation matrix.",
            ],
        ),
        ProjectError::DependenceNotFound(_) => (
            StatusCode::NOT_FOUND,
            "dependence_not_found",
            &["Set a project dependence document before trying to show or remove it."],
        ),
        ProjectError::DependenceRevisionConflict { .. } => (
            StatusCode::CONFLICT,
            "dependence_revision_conflict",
            &[
                "Show project dependence and retry with its current dependence and project revisions.",
            ],
        ),
        ProjectError::MissingEstimateAddress(_) => (
            StatusCode::BAD_REQUEST,
            "missing_estimate_address",
            &["Use estimate addresses embedded in existing project nodes or edges."],
        ),
        ProjectError::EstimateCommand(error) => estimate_error_response::classify(error),
        ProjectError::Analysis(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_analysis",
            &["Use positive cycle limits and ensure the selected scenario still exists."],
        ),
        ProjectError::ScenarioAnalysis(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "scenario_analysis_unavailable",
            &[
                "Add current estimates to every objective and causal factor used by the scenario.",
                "Remove non-empty dependence groups until correlated dynamic propagation is supported.",
            ],
        ),
        ProjectError::Yaml(_) | ProjectError::Import(_) | ProjectError::InvalidArchivePath(_) => (
            StatusCode::BAD_REQUEST,
            "invalid_project_archive",
            &["Export a fresh archive, or correct the reported YAML file and retry."],
        ),
        ProjectError::ArchiveTooManyFiles | ProjectError::ArchiveTooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "project_archive_too_large",
            &["Reduce the archive to at most 10,001 files and 32 MiB of canonical YAML."],
        ),
        ProjectError::ReplaceConfirmationRequired(_) | ProjectError::ImportProjectExists(_) => (
            StatusCode::CONFLICT,
            "project_import_requires_replace",
            &["Confirm destructive replacement explicitly before restoring over this project."],
        ),
        ProjectError::IdentifierSpaceExhausted
        | ProjectError::RevisionSpaceExhausted(_)
        | ProjectError::GraphRevisionSpaceExhausted(_)
        | ProjectError::EdgeRevisionSpaceExhausted(_)
        | ProjectError::ScenarioRevisionSpaceExhausted(_)
        | ProjectError::ScenarioIdentifierSpaceExhausted(_)
        | ProjectError::DependenceRevisionSpaceExhausted(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
        ProjectError::EvidenceCommand(
            EvidenceCommandError::IdentifierSpaceExhausted(_)
            | EvidenceCommandError::RevisionSpaceExhausted { .. },
        ) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "project_store_failure",
            &["Retry the request and inspect the server logs if the problem persists."],
        ),
        ProjectError::Repository(error) => repository_error_response::classify(error),
    }
}
