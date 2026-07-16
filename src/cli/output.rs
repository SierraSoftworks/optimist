use clap::ValueEnum;

use crate::domain::{Edge, Node, Observation, PrimitiveEstimate, ProjectDependenceModel, Scenario};
use crate::project::Project;

use super::{output_json, output_table};

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
            Self::Json | Self::Jsonl => output_json::serialize(project),
        }
    }

    pub(super) fn projects(self, projects: &[Project]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::projects(projects)),
            Self::Json => output_json::serialize(projects),
            Self::Jsonl => output_json::lines(projects),
        }
    }

    pub(super) fn node(self, node: &Node) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::nodes(std::slice::from_ref(node))),
            Self::Json | Self::Jsonl => output_json::serialize(node),
        }
    }

    pub(super) fn nodes(self, nodes: &[Node]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::nodes(nodes)),
            Self::Json => output_json::serialize(nodes),
            Self::Jsonl => output_json::lines(nodes),
        }
    }

    pub(super) fn edge(self, edge: &Edge) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::edges(std::slice::from_ref(edge))),
            Self::Json | Self::Jsonl => output_json::serialize(edge),
        }
    }

    pub(super) fn edges(self, edges: &[Edge]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::edges(edges)),
            Self::Json => output_json::serialize(edges),
            Self::Jsonl => output_json::lines(edges),
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
            Self::Json | Self::Jsonl => output_json::serialize(observation),
        }
    }

    pub(super) fn observations(
        self,
        observations: &[Observation],
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::observations(observations)),
            Self::Json => output_json::serialize(observations),
            Self::Jsonl => output_json::lines(observations),
        }
    }

    pub(super) fn scenario(self, scenario: &Scenario) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::scenarios(std::slice::from_ref(scenario))),
            Self::Json | Self::Jsonl => output_json::serialize(scenario),
        }
    }

    pub(super) fn scenarios(self, scenarios: &[Scenario]) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(output_table::scenarios(scenarios)),
            Self::Json => output_json::serialize(scenarios),
            Self::Jsonl => output_json::lines(scenarios),
        }
    }

    pub(super) fn dependence(
        self,
        model: &ProjectDependenceModel,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => Ok(format!(
                "REVISION\tGROUPS\tADDRESSES\n{}\t{}\t{}",
                model.revision,
                model.residual_groups.len(),
                model
                    .residual_groups
                    .iter()
                    .map(|group| group.members.len())
                    .sum::<usize>()
            )),
            Self::Json | Self::Jsonl => output_json::serialize(model),
        }
    }

    pub(super) fn estimate(
        self,
        estimate: &PrimitiveEstimate,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => output_table::estimate(estimate),
            Self::Json | Self::Jsonl => output_json::serialize(estimate),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            Distribution, Edge, EdgePayload, EntityId, EstimateAddress, EstimateId, EstimateOwner,
            EstimateSlot, Factor, MonteCarloConfig, Node, NodeKind, NodePayload, Observation,
            PrimitiveEstimate, ProjectId, Requirement, Scenario, ScenarioDraft, ScenarioId,
        },
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

    #[test]
    fn renders_primitive_estimates_stably() {
        let estimate = PrimitiveEstimate {
            address: EstimateAddress::new(
                ProjectId::new("A").unwrap(),
                EstimateOwner::Node(EntityId::new(0)),
                EstimateId::new(1),
            ),
            slot: EstimateSlot::Current,
            revision: 2,
            distribution: Distribution::beta(3.0, 2.0).unwrap(),
            provenance: vec!["expert".to_owned()],
        };
        assert_eq!(
            OutputFormat::Json.estimate(&estimate).unwrap(),
            r#"{"address":{"project":"A","owner":{"kind":"node","id":"A"},"estimate":"B"},"slot":{"kind":"current"},"revision":2,"distribution":{"type":"beta","alpha":3.0,"beta":2.0},"provenance":["expert"]}"#
        );
        assert_eq!(
            OutputFormat::Table.estimate(&estimate).unwrap(),
            "ADDRESS\tSLOT\tREVISION\tDISTRIBUTION\tPROVENANCE\nA/node/A/estimate/B\tCurrent\t2\t{\"type\":\"beta\",\"alpha\":3.0,\"beta\":2.0}\texpert"
        );
    }

    #[test]
    fn renders_deleted_aggregates_stably_in_every_format() {
        let node = Node::new(
            EntityId::new(0),
            "github",
            "GitHub Delivery",
            NodePayload::Factor(Factor {
                current: None,
                desired: None,
                controllable: false,
                evidence: vec![],
            }),
        )
        .unwrap();
        let edge = Edge::new(
            EntityId::new(0),
            NodeKind::Factor,
            EntityId::new(1),
            NodeKind::Factor,
            EdgePayload::Requires(Requirement {
                hard: true,
                satisfaction_threshold: None,
            }),
        )
        .unwrap();

        assert_eq!(
            OutputFormat::Table.node(&node).unwrap(),
            "ID\tNAME\tKIND\tTITLE\nA\tgithub\tfactor\tGitHub Delivery"
        );
        assert_eq!(
            OutputFormat::Table.edge(&edge).unwrap(),
            "ID\tSOURCE\tKIND\tDESTINATION\nA-requires-B\tA\trequires\tB"
        );
        assert_eq!(
            OutputFormat::Json.node(&node).unwrap(),
            OutputFormat::Jsonl.node(&node).unwrap()
        );
        assert_eq!(
            OutputFormat::Json.edge(&edge).unwrap(),
            OutputFormat::Jsonl.edge(&edge).unwrap()
        );
        let node_json: serde_json::Value =
            serde_json::from_str(&OutputFormat::Json.node(&node).unwrap()).unwrap();
        let edge_json: serde_json::Value =
            serde_json::from_str(&OutputFormat::Json.edge(&edge).unwrap()).unwrap();
        assert_eq!(node_json["id"], "A");
        assert_eq!(edge_json["source"], "A");
        assert_eq!(edge_json["destination"], "B");
    }

    #[test]
    fn renders_scenario_tables_and_jsonl_stably() {
        let scenario = Scenario::new(
            ScenarioId::new(0),
            ScenarioDraft {
                name: "delivery reliability".to_owned(),
                title: "Delivery Reliability".to_owned(),
                rationale: String::new(),
                objectives: vec![],
                planning_horizon: 12,
                budgets: vec![],
                candidate_interventions: vec![],
                monte_carlo: MonteCarloConfig::new(1, 2, 10, 0.1, 0.1).unwrap(),
                scalar_preferences: None,
            },
        )
        .unwrap();
        assert_eq!(
            OutputFormat::Table.scenario(&scenario).unwrap(),
            "ID\tNAME\tTITLE\tHORIZON\tOBJECTIVES\tCANDIDATES\tREVISION\nA\tdelivery reliability\tDelivery Reliability\t12\t0\t0\t0"
        );
        assert_eq!(
            OutputFormat::Jsonl
                .scenarios(std::slice::from_ref(&scenario))
                .unwrap(),
            OutputFormat::Json.scenario(&scenario).unwrap()
        );
    }
}
