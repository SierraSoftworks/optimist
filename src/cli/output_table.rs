use crate::{
    domain::{Edge, Node, NodeKind, Observation},
    project::Project,
};

pub(super) fn projects(projects: &[Project]) -> String {
    rows(
        "ID\tNAME\tREVISION",
        projects.iter().map(|project| {
            let name = project
                .name
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}\t{}\t{}", project.id, name, project.revision)
        }),
    )
}

pub(super) fn nodes(nodes: &[Node]) -> String {
    rows(
        "ID\tNAME\tKIND\tTITLE",
        nodes.iter().map(|node| {
            format!(
                "{}\t{}\t{}\t{}",
                node.id,
                node.name,
                node_kind(node.kind()),
                node.title.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        }),
    )
}

pub(super) fn edges(edges: &[Edge]) -> String {
    rows(
        "ID\tSOURCE\tKIND\tDESTINATION",
        edges.iter().map(|edge| {
            format!(
                "{}\t{}\t{}\t{}",
                edge.id(),
                edge.source,
                edge.payload.kind().token(),
                edge.destination
            )
        }),
    )
}

pub(super) fn observations(observations: &[Observation]) -> String {
    rows(
        "ID\tVALUE\tUNIT\tOBSERVED_AT\tSOURCE\tSUPERSEDES",
        observations.iter().map(|observation| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                observation.id,
                observation.value,
                observation.unit,
                observation.observed_at,
                observation
                    .source
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                observation
                    .supersedes
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_owned())
            )
        }),
    )
}

fn node_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Outcome => "outcome",
        NodeKind::Metric => "metric",
        NodeKind::Factor => "factor",
        NodeKind::Intervention => "intervention",
    }
}

fn rows(lines: &str, rows: impl Iterator<Item = String>) -> String {
    std::iter::once(lines.to_owned())
        .chain(rows)
        .collect::<Vec<_>>()
        .join("\n")
}
