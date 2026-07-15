use clap::ValueEnum;

use crate::domain::{Edge, Node, Observation};
use crate::project::Project;

use super::output_table;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum OutputFormat {
    Table,
    Json,
    Jsonl,
}

impl OutputFormat {
    pub(super) fn project(self, project: &Project) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::projects(std::slice::from_ref(project))),
            Self::Json | Self::Jsonl => serialize(project),
        }
    }

    pub(super) fn projects(self, projects: &[Project]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::projects(projects)),
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
            Self::Table => Ok(output_table::nodes(std::slice::from_ref(node))),
            Self::Json | Self::Jsonl => serialize(node),
        }
    }

    pub(super) fn nodes(self, nodes: &[Node]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::nodes(nodes)),
            Self::Json => serialize(nodes),
            Self::Jsonl => nodes
                .iter()
                .map(serialize)
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),
        }
    }

    pub(super) fn edge(self, edge: &Edge) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::edges(std::slice::from_ref(edge))),
            Self::Json | Self::Jsonl => serialize(edge),
        }
    }

    pub(super) fn edges(self, edges: &[Edge]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::edges(edges)),
            Self::Json => serialize(edges),
            Self::Jsonl => edges
                .iter()
                .map(serialize)
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),
        }
    }

    pub(super) fn observation(
        self,
        observation: &Observation,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::observations(std::slice::from_ref(
                observation,
            ))),
            Self::Json | Self::Jsonl => serialize(observation),
        }
    }

    pub(super) fn observations(
        self,
        observations: &[Observation],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::observations(observations)),
            Self::Json => serialize(observations),
            Self::Jsonl => observations
                .iter()
                .map(serialize)
                .collect::<Result<Vec<_>, _>>()
                .map(|lines| lines.join("\n")),
        }
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
    use crate::{
        domain::{Observation, ProjectId},
        project::Project,
    };

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

    #[test]
    fn renders_observation_history_as_a_table() {
        let observations = [Observation {
            id: 0,
            revision: 0,
            value: 0.9,
            unit: "ratio".to_owned(),
            observed_at: "2026-07-15T12:00:00Z".to_owned(),
            source: "deployment dashboard".to_owned(),
            measurement_standard_deviation: Some(0.02),
            supersedes: None,
        }];
        assert_eq!(
            OutputFormat::Table.observations(&observations).unwrap(),
            "ID\tVALUE\tUNIT\tOBSERVED_AT\tSOURCE\tSUPERSEDES\n0\t0.9\tratio\t2026-07-15T12:00:00Z\tdeployment dashboard\t-"
        );
    }
}
