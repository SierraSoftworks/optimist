use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::markdown::{
    SCHEMA_VERSION, SourceDocument, ValidatedImport, parse_entity, parse_project, parse_scenario,
};

use super::ProjectError;

const MAX_ARCHIVE_FILES: usize = 10_001;
pub(crate) const MAX_ARCHIVE_BYTES: usize = 32 * 1024 * 1024;

/// Portable JSON envelope containing canonical Markdown project files.
///
/// File contents remain byte-identical to directory export while the map gives
/// browser clients one downloadable/uploadable document without a ZIP dependency.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectArchive {
    /// Archive envelope version, independent from the Markdown schema version.
    pub schema_version: u32,
    /// Project identity and revision declared by the canonical `_project.md` file.
    pub project: crate::project::Project,
    /// Canonical project-relative Markdown files ordered by path.
    pub files: BTreeMap<String, String>,
    /// Counts useful for upload confirmation and diagnostics.
    pub summary: ProjectArchiveSummary,
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
    /// Parses and cross-validates every canonical Markdown file in this archive.
    pub fn validated_import(&self) -> Result<ValidatedImport, ProjectError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProjectError::Markdown(
                crate::markdown::MarkdownError::UnsupportedSchema {
                    path: "<archive>".to_owned(),
                    version: self.schema_version,
                },
            ));
        }
        if self.files.len() > MAX_ARCHIVE_FILES {
            return Err(ProjectError::ArchiveTooManyFiles);
        }
        let bytes = self
            .files
            .values()
            .try_fold(0_usize, |total, contents| total.checked_add(contents.len()));
        if bytes.is_none_or(|bytes| bytes > MAX_ARCHIVE_BYTES) {
            return Err(ProjectError::ArchiveTooLarge);
        }
        let project_text = self
            .files
            .get("_project.md")
            .ok_or_else(|| ProjectError::InvalidArchivePath("_project.md".to_owned()))?;
        let project =
            SourceDocument::new("_project.md", parse_project("_project.md", project_text)?);
        if project.document.project != self.project {
            return Err(ProjectError::ArchiveMetadataMismatch);
        }
        let mut entities = Vec::new();
        let mut scenarios = Vec::new();
        for (path, contents) in &self.files {
            if path == "_project.md" {
                continue;
            }
            if path.starts_with("entities/") && path.ends_with(".md") {
                let document = parse_entity(path, contents)?;
                if document.canonical_path() != *path {
                    return Err(ProjectError::InvalidArchivePath(path.clone()));
                }
                entities.push(SourceDocument::new(path, document));
            } else if path.starts_with("scenarios/") && path.ends_with(".md") {
                let document = parse_scenario(path, contents)?;
                if document.canonical_path() != *path {
                    return Err(ProjectError::InvalidArchivePath(path.clone()));
                }
                scenarios.push(SourceDocument::new(path, document));
            } else {
                return Err(ProjectError::InvalidArchivePath(path.clone()));
            }
        }
        let import = ValidatedImport::new(project, entities, scenarios)?;
        if super::project_archive_export::summary(&import) != self.summary {
            return Err(ProjectError::ArchiveMetadataMismatch);
        }
        Ok(import)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        command::{
            CommandRequest, CreateEdge, CreateNode, CreateScenario, GraphCommand, SetFormula,
            SetNodeQuantityState, SetProjectDependence, SetSquiggleEstimate,
        },
        domain::{
            CorrelationScale, Distribution, EdgePayload, EntityId, EstimateAddress,
            EstimateComponentId, EstimateId, EstimateOwner, EstimateSlot, EstimateUncertainty,
            Factor, Formula, GaussianCopulaCorrelation, Intervention, MonteCarloConfig,
            NodePayload, Outcome, OutcomeDirection, ProjectDependenceModel, QuantityDefinition,
            QuantitySupport, Requirement, ResidualDependenceGroup, ScenarioDraft,
            ScenarioObjective, Unit, UtilityDirection,
        },
    };

    use super::*;
    use crate::project::ProjectCatalog;

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
        assert_eq!(first.summary.entities, 2);
        assert_eq!(first.summary.edges, 1);
        assert!(matches!(
            catalog.import_archive(&first, false, false),
            Err(ProjectError::ImportProjectExists(_))
        ));
        assert!(matches!(
            catalog.import_archive(&first, true, false),
            Err(ProjectError::ReplaceConfirmationRequired(_))
        ));

        let mut metadata = first.clone();
        metadata.summary.entities += 1;
        assert_eq!(
            metadata.validated_import(),
            Err(ProjectError::ArchiveMetadataMismatch)
        );
        let mut path = first;
        let entity = path
            .files
            .remove("entities/A-feedback.md")
            .expect("fixture entity path");
        path.files.insert("entities/alias.md".to_owned(), entity);
        assert!(matches!(
            path.validated_import(),
            Err(ProjectError::InvalidArchivePath(_))
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
    fn restores_scenarios_formulas_and_dependence_documents() {
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
        let root = EstimateAddress::new(
            project.id.clone(),
            EstimateOwner::Node(EntityId::new(2)),
            EstimateId::new(0),
        );
        let formula = root
            .clone()
            .with_component(EstimateComponentId::new("baseline").unwrap());
        catalog
            .execute(
                &project.id,
                CommandRequest::new(
                    revision + 1,
                    GraphCommand::SetFormula(SetFormula {
                        address: formula,
                        formula: Formula::Literal {
                            distribution: Distribution::point(0.5).unwrap(),
                            unit: Unit::dimensionless(),
                        },
                        expected_revision: 0,
                        provenance: vec!["archive fixture".to_owned()],
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
                    revision + 2,
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
        catalog.delete(&project.id).unwrap();
        catalog.import_archive(&before, false, false).unwrap();
        assert_eq!(catalog.export_archive(&project.id).unwrap(), before);
        assert_eq!(catalog.list_scenarios(&project.id).unwrap().len(), 1);
        assert_eq!(catalog.list_formulas(&project.id).unwrap().revision, 1);
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
