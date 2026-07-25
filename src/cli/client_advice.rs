pub(super) fn for_error(code: &str, status: reqwest::StatusCode) -> &'static [&'static str] {
    match code {
        "invalid_project_name" => &["Provide a non-empty project name."],
        "project_name_conflict" => &["Choose a project name which is not already in use."],
        "project_not_found" => {
            &["Run `optimist project list` and retry with a returned project ID."]
        }
        "project_revision_conflict" => {
            &["Refresh the project and retry the command against its current revision."]
        }
        "invalid_command_batch" => {
            &["Submit a JSON array containing between 1 and 100 typed graph commands."]
        }
        "command_batch_not_found" => {
            &["Inspect project change replay and choose a retained forward batch ID."]
        }
        "command_batch_conflict" => {
            &["Use a fresh request ID, or inspect replay before compensating a committed batch."]
        }
        "invalid_replay_revision" => {
            &["Show the project and retry with a revision no newer than its current revision."]
        }
        "invalid_project_archive" => {
            &["Export a fresh archive, or correct the reported Markdown file and retry."]
        }
        "project_archive_too_large" => {
            &["Reduce the archive to at most 10,001 files and 32 MiB of canonical Markdown."]
        }
        "project_import_requires_replace" => {
            &["Retry with explicit replacement confirmation only after reviewing the archive."]
        }
        "backup_storage_unavailable" => {
            &["Start `optimist server` with a persistent data directory and retry."]
        }
        "backup_restore_requires_confirmation" => {
            &["Review the selected backup, then repeat the command with `--yes`."]
        }
        "backup_not_found" => {
            &["Run `optimist project backup list` and retry with a returned backup ID."]
        }
        "project_snapshot_not_found" => {
            &["Run `optimist project snapshot <PROJECT> list` and retry with a returned revision."]
        }
        "invalid_node" => &["Provide the required fields for the selected node kind."],
        "node_name_conflict" => {
            &["Choose a node name or alias which is not already used in this project."]
        }
        "node_not_found" => &["Run `optimist node list` and retry with a returned entity ID."],
        "node_has_edges" => &[
            "Run `optimist edge list`, delete every edge connected to the node, then retry `optimist node delete`.",
        ],
        "node_revision_conflict" => {
            &["Run `optimist node get <ID>` and retry against its current revision."]
        }
        "invalid_edge_id" => {
            &["Run `optimist edge list` and use a returned ID such as `A-requires-B`."]
        }
        "invalid_estimate_address" => {
            &["Use `<project>/<node|edge>/<owner>/estimate/<id>` with canonical IDs."]
        }
        "edge_conflict" => &["Use `optimist edge get` to inspect the existing relationship."],
        "edge_not_found" => &["Run `optimist edge list` and retry with a returned edge ID."],
        "edge_revision_conflict" => {
            &["Run `optimist edge get <ID>` and retry against its current revision."]
        }
        "invalid_edge" => {
            &["Check that the relationship kind is valid for both endpoint node kinds."]
        }
        "not_measurement_edge" => &["Choose a `measures` edge returned by `optimist edge list`."],
        "observation_unit_mismatch" => {
            &["Use the unit defined by the measurement edge's source metric."]
        }
        "invalid_observation" => {
            &["Check the value, RFC 3339 timestamp, source, unit, and standard deviation."]
        }
        "invalid_scenario" => &[
            "Check the scenario document fields and use positive finite importance and budget values.",
        ],
        "scenario_not_found" => {
            &["Run `optimist scenario list` and retry with a returned scenario ID."]
        }
        "scenario_revision_conflict" => {
            &["Run `optimist scenario show <ID>` and retry with its current `revision`."]
        }
        "invalid_scenario_reference" => {
            &["Use outcome IDs for objectives and intervention IDs for candidate interventions."]
        }
        "invalid_dependence" => &[
            "Check unique same-project members and the symmetric positive-semidefinite correlation matrix.",
        ],
        "dependence_not_found" => &[
            "Run `optimist dependence set --document <JSON>` before showing or removing dependence.",
        ],
        "dependence_revision_conflict" => {
            &["Run `optimist dependence show` and retry with its current `revision`."]
        }
        "missing_estimate_address" => {
            &["Use estimate addresses embedded in existing project nodes or edges."]
        }
        "cross_project_estimate_address" => {
            &["Use an address whose project ID matches `--project`."]
        }
        "invalid_estimate_slot" => {
            &["Choose a slot supported by the addressed node or edge payload."]
        }
        "estimate_conflict" => {
            &["Show the owner aggregate, then use its existing estimate ID or an unused ID."]
        }
        "estimate_not_found" => {
            &["Check the address against estimates embedded in the current node or edge payload."]
        }
        "required_estimate" => &[
            "Required causal effect and blocking degree estimates may be replaced but not removed.",
        ],
        "estimate_in_use" => {
            &["Remove project dependence which references this estimate, then retry."]
        }
        "invalid_estimate_distribution" => {
            &["Use a distribution whose complete support fits the selected slot dimension."]
        }
        "invalid_analysis" => {
            &["Use positive cycle limits and select a scenario which still exists."]
        }
        "invalid_state_relation" => &[
            "Reference only parents the graph already connects to this node, plus `baseline` and the equation's own parameters.",
            "Check the arithmetic produces the node's own unit, and keep uncertainty in named parameters rather than in the source.",
        ],
        "state_quantity_breaks_relation" => &[
            "Update the node equation that reads this quantity before changing its canonical unit terms.",
        ],
        "scenario_analysis_unavailable" => {
            &["Set current estimates on every objective and causal factor used by the scenario."]
        }
        _ if status.is_server_error() => {
            &["Retry the request and inspect server logs if it persists."]
        }
        _ => &["Check the command arguments and retry the request."],
    }
}
