use clap::ValueEnum;

use crate::domain::{
    Edge, FormulaCatalog, FormulaDefinition, Node, Observation, PrimitiveEstimate,
    ProjectDependenceModel, Scenario, ScenarioAnalysis,
};
use crate::project::Project;

use super::{output_json, output_scenario_analysis, output_table, output_table_formula};

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

    pub(super) fn scenario_analysis(
        self,
        analysis: &ScenarioAnalysis,
    ) -> Result<String, human_errors::Error> {
        output_scenario_analysis::render(self, analysis)
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

    pub(super) fn formula(
        self,
        formula: &FormulaDefinition,
    ) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => output_table_formula::render(0, std::slice::from_ref(formula)),
            Self::Json | Self::Jsonl => output_json::serialize(formula),
        }
    }

    pub(super) fn formulas(self, catalog: &FormulaCatalog) -> Result<String, human_errors::Error> {
        match self {
            Self::Table => output_table_formula::render(catalog.revision, &catalog.formulas),
            Self::Json => output_json::serialize(catalog),
            Self::Jsonl => output_json::lines(&catalog.formulas),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            CompiledFormula, Distribution, Edge, EdgePayload, EntityId, EstimateAddress,
            EstimateComponentId, EstimateId, EstimateOwner, EstimateSlot, Factor, Formula,
            FormulaCatalog, FormulaDefinition, InterventionProjection, InvalidSampleCounts,
            MonteCarloConfig, MonteCarloDiagnostics, MonteCarloEstimate, Node, NodeKind,
            NodePayload, ObjectiveProjection, Observation, PrimitiveEstimate, ProjectId,
            Requirement, Scenario, ScenarioAnalysis, ScenarioDraft, ScenarioId, Unit,
            UtilityDirection,
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
            quantity: None,
            source: crate::domain::EstimateSource::Distribution,
            provenance: vec!["expert".to_owned()],
            uncertainty: crate::domain::EstimateUncertainty::new(
                "limited evidence",
                "weekly variation",
                "sampling error",
            )
            .unwrap(),
        };
        assert_eq!(
            OutputFormat::Json.estimate(&estimate).unwrap(),
            r#"{"address":{"project":"A","owner":{"kind":"node","id":"A"},"estimate":"B"},"slot":{"kind":"current"},"revision":2,"distribution":{"type":"beta","alpha":3.0,"beta":2.0},"source":{"type":"distribution"},"provenance":["expert"],"uncertainty":{"epistemic":"limited evidence","process":"weekly variation","measurement":"sampling error"}}"#
        );
        assert_eq!(
            OutputFormat::Table.estimate(&estimate).unwrap(),
            "ADDRESS\tSLOT\tREVISION\tSOURCE\tDISTRIBUTION\tQUANTITY\tPROVENANCE\tEPISTEMIC\tPROCESS\tMEASUREMENT\nA/node/A/estimate/B\tCurrent\t2\tdistribution\t{\"type\":\"beta\",\"alpha\":3.0,\"beta\":2.0}\t-\texpert\tlimited evidence\tweekly variation\tsampling error"
        );
    }

    #[test]
    fn renders_formula_catalogs_stably() {
        let root = EstimateAddress::new(
            ProjectId::new("A").unwrap(),
            EstimateOwner::Node(EntityId::new(0)),
            EstimateId::new(0),
        );
        let formula = FormulaDefinition {
            address: root
                .clone()
                .with_component(EstimateComponentId::new("base").unwrap()),
            formula: Formula::Reference {
                address: root.clone(),
            },
            compiled: CompiledFormula {
                unit: Unit::dimensionless(),
                dependencies: vec![root],
            },
            provenance: vec!["expert".to_owned()],
        };
        let catalog = FormulaCatalog {
            revision: 2,
            formulas: vec![formula],
        };
        assert_eq!(
            OutputFormat::Table.formulas(&catalog).unwrap(),
            "DOCUMENT_REVISION\tADDRESS\tUNIT\tDEPENDENCIES\tPROVENANCE\n2\tA/node/A/estimate/A/component/base\t1\t1\texpert"
        );
        assert!(
            OutputFormat::Json
                .formulas(&catalog)
                .unwrap()
                .contains("\"revision\":2")
        );
        assert!(
            !OutputFormat::Jsonl
                .formulas(&catalog)
                .unwrap()
                .contains("\"revision\":2")
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

    #[test]
    fn renders_empty_scenario_analysis_stably() {
        let analysis = ScenarioAnalysis {
            revision: crate::domain::AnalysisRevisionKey {
                project: ProjectId::new("A").unwrap(),
                graph_revision: 2,
                scenario: Some((ScenarioId::new(0), 1)),
                dependence_revision: None,
                formula_revision: 0,
            },
            planning_horizon: 4,
            candidates: vec![],
        };
        assert_eq!(
            OutputFormat::Table.scenario_analysis(&analysis).unwrap(),
            "INTERVENTION\tOUTCOME\tREACHABLE\tDIRECTION\tIMPORTANCE\tBASELINE_MEAN\tFINAL_MEAN\tIMPROVEMENT_MEAN\tIMPROVEMENT_VARIANCE\tCLAMPED_UPDATES\tSAMPLES\tSTATUS"
        );
        assert_eq!(
            OutputFormat::Jsonl.scenario_analysis(&analysis).unwrap(),
            ""
        );
        assert!(
            OutputFormat::Json
                .scenario_analysis(&analysis)
                .unwrap()
                .contains("\"planning_horizon\":4")
        );
    }

    #[test]
    fn renders_candidate_analysis_rows_stably() {
        let config = MonteCarloConfig::new(1, 2, 2, 0.1, 0.0).unwrap();
        let estimate = MonteCarloEstimate {
            mean: Some(0.5),
            variance: Some(0.0),
            mean_standard_error: Some(0.0),
            variance_standard_error: None,
        };
        let analysis = ScenarioAnalysis {
            revision: crate::domain::AnalysisRevisionKey {
                project: ProjectId::new("A").unwrap(),
                graph_revision: 2,
                scenario: Some((ScenarioId::new(0), 1)),
                dependence_revision: None,
                formula_revision: 0,
            },
            planning_horizon: 4,
            candidates: vec![InterventionProjection {
                intervention: EntityId::new(1),
                objectives: vec![ObjectiveProjection {
                    outcome: EntityId::new(0),
                    direction: UtilityDirection::Maximize,
                    importance: 1.0,
                    reachable: true,
                    baseline: estimate.clone(),
                    final_state: estimate.clone(),
                    improvement: estimate,
                    trajectory: vec![],
                }],
                improvement_covariance: vec![vec![Some(0.0)]],
                clamped_state_updates: 3,
                diagnostics: MonteCarloDiagnostics {
                    seed: 1,
                    attempted_samples: 2,
                    valid_samples: 2,
                    invalid_samples: InvalidSampleCounts::default(),
                    criterion: config,
                    status: crate::domain::ConvergenceStatus::Converged,
                },
            }],
        };
        let table = OutputFormat::Table.scenario_analysis(&analysis).unwrap();
        assert!(table.contains("B\tA\ttrue\tMaximize"));
        assert!(table.contains("\t3\t2\tConverged"));
        let jsonl = OutputFormat::Jsonl.scenario_analysis(&analysis).unwrap();
        assert!(jsonl.contains("\"reachable\":true"));
        assert!(jsonl.contains("\"clamped_state_updates\":3"));
    }
}
