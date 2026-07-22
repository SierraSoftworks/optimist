use crate::{
    domain::{Edge, Node, NodeKind, Observation, PrimitiveEstimate, Scenario},
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

pub(super) fn scenarios(scenarios: &[Scenario]) -> String {
    rows(
        "ID\tNAME\tTITLE\tHORIZON\tOBJECTIVES\tCANDIDATES\tREVISION",
        scenarios.iter().map(|scenario| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                scenario.id,
                scenario
                    .draft
                    .name
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                scenario
                    .draft
                    .title
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                scenario.draft.planning_horizon,
                scenario.draft.objectives.len(),
                scenario.draft.candidate_interventions.len(),
                scenario.revision,
            )
        }),
    )
}

pub(super) fn estimate(estimate: &PrimitiveEstimate) -> Result<String, human_errors::Error> {
    let source = match &estimate.source {
        crate::domain::EstimateSource::Distribution => "distribution".to_owned(),
        crate::domain::EstimateSource::Fermi { definition, .. } => {
            format!("fermi:{}", definition.equation.replace(['\t', '\n'], " "))
        }
        crate::domain::EstimateSource::Squiggle { definition, .. } => {
            format!("squiggle:{}", definition.source.replace(['\t', '\n'], " "))
        }
    };
    Ok(format!(
        "ADDRESS\tSLOT\tREVISION\tSOURCE\tDISTRIBUTION\tPROVENANCE\n{}\t{:?}\t{}\t{}\t{}\t{}",
        estimate.address,
        estimate.slot,
        estimate.revision,
        source,
        super::output_json::serialize(&estimate.distribution)?,
        estimate.provenance.join("; ")
    ))
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
