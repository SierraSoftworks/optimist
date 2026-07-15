use clap::ValueEnum;

use crate::domain::{Node, NodeKind};
use crate::project::Project;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    Table,
    Json,
    Jsonl,
}

impl OutputFormat {
    pub(super) fn project(self, project: &Project) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(table(std::slice::from_ref(project))),
            Self::Json | Self::Jsonl => serialize(project),
        }
    }

    pub(super) fn projects(self, projects: &[Project]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(table(projects)),
            Self::Json => serialize(projects),
            Self::Jsonl => projects
                .iter()
                .map(serialize)
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),
        }
    }

    pub(super) fn node(self, node: &Node) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(node_table(std::slice::from_ref(node))),
            Self::Json | Self::Jsonl => serialize(node),
        }
    }

    pub(super) fn nodes(self, nodes: &[Node]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(node_table(nodes)),
            Self::Json => serialize(nodes),
            Self::Jsonl => nodes
                .iter()
                .map(serialize)
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),
        }
    }
}

fn table(projects: &[Project]) -> String {
    std::iter::once("ID\tNAME\tREVISION".to_owned())
        .chain(projects.iter().map(|project| {
            let name = project
                .name
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            format!("{}\t{}\t{}", project.id, name, project.revision)
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn node_table(nodes: &[Node]) -> String {
    std::iter::once("ID\tNAME\tKIND\tTITLE".to_owned())
        .chain(nodes.iter().map(|node| {
            format!(
                "{}\t{}\t{}\t{}",
                node.id,
                node.name,
                node_kind(node.kind()),
                node.title.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn node_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Outcome => "outcome",
        NodeKind::Metric => "metric",
        NodeKind::Factor => "factor",
        NodeKind::Intervention => "intervention",
    }
}

fn serialize<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, human_errors::Error> {
    serde_json::to_string(value).map_err(|error| {
        human_errors::wrap_system(
            error,
            "Optimist could not serialize command output.",
            &["Retry with `--output table` and report the serialization failure if it persists."],
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::{domain::ProjectId, project::Project};

    use super::OutputFormat;

    fn projects() -> Vec<Project> {
        vec![
            Project {
                id: ProjectId::new("A").unwrap(),
                name: "Delivery\nHealth".to_owned(),
                revision: 0,
            },
            Project {
                id: ProjectId::new("B").unwrap(),
                name: "Security".to_owned(),
                revision: 2,
            },
        ]
    }

    #[test]
    fn renders_stable_table_output() {
        assert_eq!(
            OutputFormat::Table.projects(&projects()).unwrap(),
            "ID\tNAME\tREVISION\nA\tDelivery Health\t0\nB\tSecurity\t2"
        );
    }

    #[test]
    fn renders_single_project_as_json_object() {
        assert_eq!(
            OutputFormat::Json.project(&projects()[0]).unwrap(),
            r#"{"id":"A","name":"Delivery\nHealth","revision":0}"#
        );
    }

    #[test]
    fn renders_project_lists_as_jsonl() {
        assert_eq!(
            OutputFormat::Jsonl.projects(&projects()).unwrap(),
            "{\"id\":\"A\",\"name\":\"Delivery\\nHealth\",\"revision\":0}\n{\"id\":\"B\",\"name\":\"Security\",\"revision\":2}"
        );
    }
}
