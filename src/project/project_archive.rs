use serde::{Deserialize, Serialize};

use crate::project_yaml::{
    EntityDocument, ProjectDocument, SCHEMA_VERSION, ScenarioDocument, SourceDocument,
    ValidatedImport, render_entity, render_project, render_scenario,
};

use super::ProjectError;

const MAX_ARCHIVE_FILES: usize = 10_001;
pub(crate) const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;

/// Portable project structure serialized directly as YAML.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectArchive {
    /// YAML project schema version.
    pub schema_version: u32,
    /// Project identity and revision.
    pub project: crate::project::Project,
    /// Project rationale and scope.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Optional project-level Gaussian residual dependence document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependence: Option<crate::domain::ProjectDependenceModel>,
    /// Complete entity documents ordered by project-local identity.
    #[serde(default)]
    pub entities: Vec<EntityDocument>,
    /// Complete scenario documents ordered by project-local identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenarios: Vec<ScenarioDocument>,
}

/// Aggregate counts retained beside one portable project archive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectArchiveSummary {
    /// Project-local entity documents in the archive.
    pub entities: usize,
    /// Structural relationships embedded in entity documents.
    pub edges: usize,
    /// Scenario documents in the archive.
    pub scenarios: usize,
}

impl ProjectArchive {
    /// Returns aggregate counts derived from the typed project structure.
    pub fn summary(&self) -> ProjectArchiveSummary {
        ProjectArchiveSummary {
            entities: self.entities.len(),
            edges: self
                .entities
                .iter()
                .map(|entity| entity.outgoing_edges.len())
                .sum(),
            scenarios: self.scenarios.len(),
        }
    }

    /// Validates every YAML document and all cross-document references.
    pub fn validated_import(&self) -> Result<ValidatedImport, ProjectError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProjectError::Yaml(
                crate::project_yaml::YamlError::UnsupportedSchema {
                    path: "<archive>".to_owned(),
                    version: self.schema_version,
                },
            ));
        }
        if 1 + self.entities.len() + self.scenarios.len() > MAX_ARCHIVE_FILES {
            return Err(ProjectError::ArchiveTooManyFiles);
        }
        let bytes = serde_yaml_ng::to_string(self)
            .map_err(|error| crate::project_yaml::YamlError::Render(error.to_string()))?
            .len();
        if bytes > MAX_ARCHIVE_BYTES {
            return Err(ProjectError::ArchiveTooLarge);
        }
        let project_document = ProjectDocument {
            schema_version: self.schema_version,
            project: self.project.clone(),
            dependence: self.dependence.clone(),
            description: self.description.clone(),
        };
        render_project(&project_document)?;
        let entities = self
            .entities
            .iter()
            .cloned()
            .map(|document| {
                render_entity(&document)?;
                Ok(SourceDocument::new(document.canonical_path(), document))
            })
            .collect::<Result<Vec<_>, ProjectError>>()?;
        let scenarios = self
            .scenarios
            .iter()
            .cloned()
            .map(|document| {
                render_scenario(&document)?;
                Ok(SourceDocument::new(document.canonical_path(), document))
            })
            .collect::<Result<Vec<_>, ProjectError>>()?;
        Ok(ValidatedImport::new(
            SourceDocument::new("_project.yaml", project_document),
            entities,
            scenarios,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandRequest, CreateEdge, CreateNode, CreateScenario, GraphCommand,
            SetNodeQuantityState, SetProjectDependence, SetSquiggleEstimate,
        },
        domain::{
            CorrelationScale, EdgePayload, EntityId, EstimateAddress, EstimateId, EstimateOwner,
            EstimateSlot, EstimateUncertainty, Factor, GaussianCopulaCorrelation, Intervention,
            MonteCarloConfig, NodePayload, Outcome, OutcomeDirection, ProjectDependenceModel,
            QuantityDefinition, QuantitySupport, Requirement, ResidualDependenceGroup,
            ScenarioDraft, ScenarioObjective, Unit, UtilityDirection,
        },
    };

    use super::*;
    use crate::project::ProjectCatalog;

    fn contains_yaml_key(value: &serde_yaml_ng::Value, key: &str) -> bool {
        match value {
            serde_yaml_ng::Value::Mapping(values) => values.iter().any(|(candidate, value)| {
                candidate.as_str() == Some(key)
                    || contains_yaml_key(candidate, key)
                    || contains_yaml_key(value, key)
            }),
            serde_yaml_ng::Value::Sequence(values) => {
                values.iter().any(|value| contains_yaml_key(value, key))
            }
            _ => false,
        }
    }

    fn populated_catalog() -> (ProjectCatalog, crate::domain::ProjectId) {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Delivery".to_owned()).unwrap();
        for (revision, name) in [(0, "feedback"), (1, "learning")] {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::CreateNode(CreateNode {
                            name: name.to_owned(),
                            title: name.to_owned(),
                            payload: NodePayload::Factor(Factor {
                                controllable: true,
                                evidence: vec![],
                            }),
                        }),
                    ),
                )
                .unwrap();
        }
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    2,
                    GraphCommand::CreateEdge(CreateEdge {
                        source: crate::domain::EntityId::new(0),
                        destination: crate::domain::EntityId::new(1),
                        payload: EdgePayload::Requires(Requirement {
                            hard: true,
                            satisfaction_threshold: None,
                        }),
                    }),
                ),
            )
            .unwrap();
        (catalog, project.id)
    }

    #[test]
    fn export_is_deterministic_and_replacement_requires_confirmation() {
        let (mut catalog, project) = populated_catalog();
        let first = catalog.export_archive(&project).unwrap();
        let second = catalog.export_archive(&project).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.summary().entities, 2);
        assert_eq!(first.summary().edges, 1);
        assert!(matches!(
            catalog.import_archive(&first, false, false),
            Err(ProjectError::ImportProjectExists(_))
        ));
        assert!(matches!(
            catalog.import_archive(&first, true, false),
            Err(ProjectError::ReplaceConfirmationRequired(_))
        ));

        let mut unsupported = first;
        unsupported.schema_version += 1;
        assert!(matches!(
            unsupported.validated_import(),
            Err(ProjectError::Yaml(_))
        ));
    }

    #[test]
    fn delete_import_export_is_byte_stable_and_resets_replay_floor() {
        let (mut catalog, project) = populated_catalog();
        let before = catalog.export_archive(&project).unwrap();
        catalog.delete(&project).unwrap();
        let restored = catalog.import_archive(&before, false, false).unwrap();
        assert_eq!(restored.id, project);
        assert_eq!(catalog.export_archive(&project).unwrap(), before);
        assert_eq!(catalog.create("Next".to_owned()).unwrap().id.as_str(), "B");
        assert!(matches!(
            catalog.replay_changes(&project, 0),
            Err(ProjectError::ChangeHistoryGap {
                available_after,
                ..
            }) if available_after == restored.revision
        ));
        assert!(
            catalog
                .replay_changes(&project, restored.revision)
                .unwrap()
                .changes
                .is_empty()
        );

        let result = catalog
            .execute(
                &project,
                CommandRequest::new(
                    restored.revision,
                    GraphCommand::CreateNode(CreateNode {
                        name: "throughput".to_owned(),
                        title: "Throughput".to_owned(),
                        payload: NodePayload::Factor(Factor {
                            controllable: false,
                            evidence: vec![],
                        }),
                    }),
                ),
            )
            .unwrap();
        let crate::command::CommandOutcome::NodeCreated(node) = result.outcome else {
            panic!("expected created node")
        };
        assert_eq!(node.id, crate::domain::EntityId::new(2));
    }

    #[test]
    fn restores_scenarios_dependence_and_squiggle_sources() {
        let mut catalog = ProjectCatalog::new();
        let project = catalog.create("Complete".to_owned()).unwrap();
        let payloads = [
            NodePayload::Outcome(Outcome {
                direction: OutcomeDirection::Maximize,
                evidence: vec![],
            }),
            NodePayload::Intervention(Intervention {
                costs: vec![],
                duration: None,
                probability_of_success: None,
                acceptance_criteria: vec![],
            }),
            NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
            NodePayload::Factor(Factor {
                controllable: false,
                evidence: vec![],
            }),
        ];
        for (revision, payload) in payloads.into_iter().enumerate() {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision as u64,
                        GraphCommand::CreateNode(CreateNode {
                            name: format!("node-{revision}"),
                            title: format!("Node {revision}"),
                            payload,
                        }),
                    ),
                )
                .unwrap();
        }
        let quantity = QuantityDefinition::with_dimension(
            "state",
            Some(Unit::dimensionless()),
            None,
            QuantitySupport::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
        let mut revision = 4;
        for node in [0, 2, 3] {
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::SetNodeQuantityState(SetNodeQuantityState {
                            node: EntityId::new(node),
                            expected_revision: 0,
                            quantity: quantity.clone(),
                        }),
                    ),
                )
                .unwrap();
            revision += 1;
            catalog
                .execute(
                    &project.id,
                    CommandRequest::new(
                        revision,
                        GraphCommand::SetSquiggleEstimate(SetSquiggleEstimate {
                            address: EstimateAddress::new(
                                project.id.clone(),
                                EstimateOwner::Node(EntityId::new(node)),
                                EstimateId::new(0),
                            ),
                            slot: EstimateSlot::Current,
                            definition: crate::domain::SquiggleEstimateDefinition {
                                source: "beta(2, 2)".to_owned(),
                                seed: 42,
                                sample_count: 256,
                                target_unit: Unit::dimensionless(),
                            },
                            provenance: vec![],
                            uncertainty: EstimateUncertainty::default(),
                        }),
                    ),
                )
                .unwrap();
            revision += 1;
        }
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    revision,
                    GraphCommand::CreateScenario(CreateScenario {
                        scenario: ScenarioDraft {
                            name: "plan".to_owned(),
                            title: "Plan".to_owned(),
                            rationale: "Restore this scenario.".to_owned(),
                            objectives: vec![ScenarioObjective {
                                outcome_id: EntityId::new(0),
                                direction: UtilityDirection::Maximize,
                                importance: 1.0,
                            }],
                            planning_horizon: 4,
                            budgets: vec![],
                            candidate_interventions: vec![EntityId::new(1)],
                            monte_carlo: MonteCarloConfig::new(7, 10, 100, 0.01, 0.01).unwrap(),
                            scalar_preferences: None,
                        },
                    }),
                ),
            )
            .unwrap();
        let member = |id| {
            EstimateAddress::new(
                project.id.clone(),
                EstimateOwner::Node(EntityId::new(id)),
                EstimateId::new(0),
            )
        };
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    revision + 1,
                    GraphCommand::SetProjectDependence(SetProjectDependence {
                        model: ProjectDependenceModel {
                            revision: 0,
                            residual_groups: vec![ResidualDependenceGroup {
                                members: vec![member(2), member(3)],
                                correlation: GaussianCopulaCorrelation {
                                    scale: CorrelationScale::Latent,
                                    matrix: vec![vec![1.0, 0.25], vec![0.25, 1.0]],
                                },
                            }],
                        },
                    }),
                ),
            )
            .unwrap();

        let before = catalog.export_archive(&project.id).unwrap();
        let yaml = serde_yaml_ng::to_string(&before).unwrap();
        assert!(yaml.contains("source: beta(2, 2)"));
        assert!(yaml.contains("seed: 42"));
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        for derived in ["distribution:", "samples:", "assessment:", "p50:", "p90:"] {
            let key = derived.trim_end_matches(':');
            assert!(
                !contains_yaml_key(&value, key),
                "persisted derived field {key}"
            );
        }
        catalog.delete(&project.id).unwrap();
        catalog.import_archive(&before, false, false).unwrap();
        assert_eq!(catalog.export_archive(&project.id).unwrap(), before);
        assert_eq!(catalog.list_scenarios(&project.id).unwrap().len(), 1);
        assert_eq!(
            catalog
                .get_dependence(&project.id)
                .unwrap()
                .unwrap()
                .revision,
            0
        );
    }
}
