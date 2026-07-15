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
        "invalid_node" => &["Provide the required fields for the selected node kind."],
        "node_name_conflict" => {
            &["Choose a node name or alias which is not already used in this project."]
        }
        "node_not_found" => &["Run `optimist node list` and retry with a returned entity ID."],
        "node_has_edges" => &[
            "Run `optimist edge list`, delete every edge connected to the node, then retry `optimist node delete`.",
        ],
        "invalid_edge_id" => {
            &["Run `optimist edge list` and use a returned ID such as `A-requires-B`."]
        }
        "edge_conflict" => &["Use `optimist edge get` to inspect the existing relationship."],
        "edge_not_found" => &["Run `optimist edge list` and retry with a returned edge ID."],
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
        _ if status.is_server_error() => {
            &["Retry the request and inspect server logs if it persists."]
        }
        _ => &["Check the command arguments and retry the request."],
    }
}
