use crate::{
    domain::{
        CorrelationScale, Edge, EdgePayload, EntityId, EstimateAddress, EstimateId, EstimateOwner,
        Factor, GaussianCopulaCorrelation, Intervention, MonteCarloConfig, Node, NodeKind,
        NodePayload, Outcome, OutcomeDirection, ProjectDependenceModel, ResidualDependenceGroup,
        Scenario, ScenarioDraft, ScenarioId, ScenarioObjective, UtilityDirection,
    },
    markdown::{
        EntityDocument, ImportError, MarkdownError, MergeAction, MergeConflict, MergePlan,
        ProjectDocument, RenderedSnapshot, SCHEMA_VERSION, ScenarioDocument, SourceDocument,
        ValidatedImport, parse_entity, parse_project, parse_scenario, read_directory,
        render_entity, render_project, render_scenario, write_directory,
    },
    project::Project,
};

fn node(id: u64, name: &str) -> Node {
    let mut node = Node::new(
        EntityId::new(id),
        name,
        name,
        NodePayload::Factor(Factor {
            controllable: true,
            evidence: vec![],
        }),
    )
    .unwrap();
    node.description = format!("# {name}\n\nNarrative.\n");
    node
}

fn edge(source: u64, destination: u64) -> Edge {
    Edge::new(
        EntityId::new(source),
        NodeKind::Factor,
        EntityId::new(destination),
        NodeKind::Factor,
        EdgePayload::PartOf,
    )
    .unwrap()
}

#[test]
fn entity_render_is_deterministic_and_semantically_stable() {
    let mut relationship = edge(0, 2);
    relationship.description = "# Composition\n\nDelivery includes quality.".to_owned();
    relationship
        .metadata
        .insert("source".to_owned(), serde_json::json!("ADR-1"));
    let document = EntityDocument {
        schema_version: SCHEMA_VERSION,
        base_project_revision: 7,
        node: node(0, "delivery"),
        outgoing_edges: vec![relationship, edge(0, 1)],
    };
    let first = render_entity(&document).unwrap();
    let second = render_entity(&document).unwrap();
    assert_eq!(first, second);
    assert!(!first.contains('\r'));
    let parsed = parse_entity("entities/A-delivery.md", &first).unwrap();
    assert_eq!(parsed.outgoing_edges[0].destination, EntityId::new(1));
    assert_eq!(
        parsed.outgoing_edges[1].description,
        "# Composition\n\nDelivery includes quality."
    );
    assert_eq!(parsed.outgoing_edges[1].metadata["source"], "ADR-1");
    assert_eq!(render_entity(&parsed).unwrap(), first);
}

#[test]
fn native_state_round_trips() {
    let mut document = entity_document(node(0, "lead-time"), 1).document;
    let quantity = crate::domain::QuantityDefinition::with_dimension(
        "days",
        Some(crate::domain::Unit::base("day").unwrap()),
        None,
        crate::domain::QuantitySupport::NonNegative,
    )
    .unwrap();
    document.node.native_state =
        Some(crate::domain::QuantityState::new(quantity, None, None).unwrap());
    let rendered = render_entity(&document).unwrap();
    let parsed = parse_entity("entities/A.md", &rendered).unwrap();
    assert_eq!(parsed.node.native_state, document.node.native_state);
}

#[test]
fn project_body_is_separate_from_frontmatter() {
    let project_id = crate::domain::ProjectId::new("A").unwrap();
    let document = ProjectDocument {
        schema_version: SCHEMA_VERSION,
        project: Project {
            id: project_id.clone(),
            name: "Delivery".to_owned(),
            revision: 3,
        },
        dependence: Some(ProjectDependenceModel {
            revision: 2,
            residual_groups: vec![ResidualDependenceGroup {
                members: vec![
                    EstimateAddress::new(
                        project_id.clone(),
                        EstimateOwner::Node(EntityId::new(0)),
                        EstimateId::new(0),
                    ),
                    EstimateAddress::new(
                        project_id,
                        EstimateOwner::Node(EntityId::new(1)),
                        EstimateId::new(0),
                    ),
                ],
                correlation: GaussianCopulaCorrelation {
                    scale: CorrelationScale::Rank,
                    matrix: vec![vec![1.0, 0.5], vec![0.5, 1.0]],
                },
            }],
        }),
        formulas: crate::domain::FormulaDocument::default(),
        description: "# Delivery\n\nScope.\n".to_owned(),
    };
    let rendered = render_project(&document).unwrap();
    assert_eq!(parse_project("_project.md", &rendered).unwrap(), document);
    assert_eq!(render_project(&document).unwrap(), rendered);
}

#[test]
fn rejects_foreign_and_duplicate_outgoing_edges() {
    let foreign = EntityDocument {
        schema_version: SCHEMA_VERSION,
        base_project_revision: 0,
        node: node(0, "delivery"),
        outgoing_edges: vec![edge(1, 2)],
    };
    assert!(matches!(
        render_entity(&foreign),
        Err(MarkdownError::ForeignOutgoingEdge { .. })
    ));

    let duplicate = EntityDocument {
        outgoing_edges: vec![edge(0, 1), edge(0, 1)],
        ..foreign
    };
    assert!(matches!(
        render_entity(&duplicate),
        Err(MarkdownError::DuplicateEdge { .. })
    ));
}

#[test]
fn reports_yaml_source_location_and_schema_errors() {
    let yaml = "---\nschema_version: 1\nproject: [\n---\n";
    let Err(MarkdownError::InvalidYaml {
        path, line, column, ..
    }) = parse_project("_project.md", yaml)
    else {
        panic!("expected a source-aware YAML error")
    };
    assert_eq!(path, "_project.md");
    assert!(line >= 2);
    assert!(column >= 1);
    let unsupported =
        "---\nschema_version: 99\nproject:\n  id: A\n  name: Delivery\n  revision: 0\n---\n";
    assert_eq!(
        parse_project("_project.md", unsupported),
        Err(MarkdownError::UnsupportedSchema {
            path: "_project.md".to_owned(),
            version: 99,
        })
    );
}

#[test]
fn rejects_noncanonical_or_oversized_documents() {
    assert_eq!(
        parse_project("_project.md", "---\r\nschema_version: 1\r\n---\r\n"),
        Err(MarkdownError::NonCanonicalLineEndings(
            "_project.md".to_owned()
        ))
    );
    let oversized = "x".repeat(1024 * 1024 + 1);
    assert!(matches!(
        parse_project("_project.md", &oversized),
        Err(MarkdownError::DocumentTooLarge { .. })
    ));

    let dense_frontmatter = format!(
        "---\n{}schema_version: 1\nproject:\n  id: A\n  name: Delivery\n  revision: 0\n---\n",
        "# retained empirical data\n".repeat(12_000)
    );
    assert!(dense_frontmatter.len() > 256 * 1024);
    assert!(parse_project("_project.md", &dense_frontmatter).is_ok());

    let oversized_frontmatter = format!(
        "---\n{}\n---\n",
        "# bounded structured data\n".repeat(21_000)
    );
    assert!(oversized_frontmatter.len() < 1024 * 1024);
    assert!(matches!(
        parse_project("_project.md", &oversized_frontmatter),
        Err(MarkdownError::FrontmatterTooLarge { .. })
    ));
}

#[test]
fn scenario_render_is_canonical_and_semantically_stable() {
    let document = ScenarioDocument {
        schema_version: SCHEMA_VERSION,
        base_project_revision: 8,
        scenario: Scenario::new(
            ScenarioId::new(0),
            ScenarioDraft {
                name: "Delivery Reliability".to_owned(),
                title: "Reliable delivery".to_owned(),
                rationale: "# Decision\n\nPrefer sustainable improvements.\n".to_owned(),
                objectives: vec![],
                planning_horizon: 12,
                budgets: vec![],
                candidate_interventions: vec![],
                monte_carlo: MonteCarloConfig::new(42, 10, 100, 0.01, 0.01).unwrap(),
                scalar_preferences: None,
            },
        )
        .unwrap(),
    };
    assert_eq!(
        document.canonical_path(),
        "scenarios/A-delivery-reliability.md"
    );
    let rendered = render_scenario(&document).unwrap();
    assert!(!rendered.contains("rationale:"));
    let parsed = parse_scenario(document.canonical_path(), &rendered).unwrap();
    assert_eq!(parsed, document);
    assert_eq!(render_scenario(&parsed).unwrap(), rendered);
}

fn project_document(revision: u64) -> ProjectDocument {
    ProjectDocument {
        schema_version: SCHEMA_VERSION,
        project: Project {
            id: crate::domain::ProjectId::new("A").unwrap(),
            name: "Delivery".to_owned(),
            revision,
        },
        dependence: None,
        formulas: crate::domain::FormulaDocument::default(),
        description: String::new(),
    }
}

fn entity_document(node: Node, revision: u64) -> SourceDocument<EntityDocument> {
    let path = format!("entities/{}.md", node.id);
    SourceDocument::new(
        path,
        EntityDocument {
            schema_version: SCHEMA_VERSION,
            base_project_revision: revision,
            node,
            outgoing_edges: vec![],
        },
    )
}

#[test]
fn validates_complete_snapshot_references_in_a_second_pass() {
    let outcome = Node::new(
        EntityId::new(0),
        "reliability",
        "Reliability",
        NodePayload::Outcome(Outcome {
            direction: OutcomeDirection::Maximize,
            evidence: vec![],
        }),
    )
    .unwrap();
    let intervention = Node::new(
        EntityId::new(1),
        "automation",
        "Automation",
        NodePayload::Intervention(Intervention {
            costs: vec![],
            duration: None,
            probability_of_success: None,
            acceptance_criteria: vec![],
        }),
    )
    .unwrap();
    let scenario = Scenario::new(
        ScenarioId::new(0),
        ScenarioDraft {
            name: "plan".to_owned(),
            title: "Plan".to_owned(),
            rationale: String::new(),
            objectives: vec![ScenarioObjective {
                outcome_id: outcome.id,
                direction: UtilityDirection::Maximize,
                importance: 1.0,
            }],
            planning_horizon: 4,
            budgets: vec![],
            candidate_interventions: vec![intervention.id],
            monte_carlo: MonteCarloConfig::new(1, 10, 20, 0.1, 0.1).unwrap(),
            scalar_preferences: None,
        },
    )
    .unwrap();
    let validated = ValidatedImport::new(
        SourceDocument::new("_project.md", project_document(3)),
        vec![
            entity_document(outcome, 3),
            entity_document(intervention, 3),
        ],
        vec![SourceDocument::new(
            "scenarios/A-plan.md",
            ScenarioDocument {
                schema_version: SCHEMA_VERSION,
                base_project_revision: 3,
                scenario,
            },
        )],
    )
    .unwrap();
    assert_eq!(validated.entities.len(), 2);
    assert_eq!(validated.scenarios.len(), 1);
}

#[test]
fn rejects_duplicate_names_and_inconsistent_revisions() {
    let mut alias = node(1, "other");
    alias.aliases.push("DELIVERY".to_owned());
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project_document(2)),
            vec![
                entity_document(node(0, "delivery"), 2),
                entity_document(alias, 2)
            ],
            vec![],
        ),
        Err(ImportError::DuplicateNodeName { .. })
    ));
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project_document(2)),
            vec![entity_document(node(0, "delivery"), 1)],
            vec![],
        ),
        Err(ImportError::InconsistentBaseRevision { .. })
    ));
}

#[test]
fn rejects_missing_edge_and_wrong_scenario_reference_kind() {
    let mut source = entity_document(node(0, "delivery"), 0);
    source.document.outgoing_edges.push(edge(0, 1));
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project_document(0)),
            vec![source],
            vec![],
        ),
        Err(ImportError::MissingEdgeEndpoint { .. })
    ));

    let factor = node(0, "delivery");
    let scenario = Scenario::new(
        ScenarioId::new(0),
        ScenarioDraft {
            name: "plan".to_owned(),
            title: "Plan".to_owned(),
            rationale: String::new(),
            objectives: vec![ScenarioObjective {
                outcome_id: factor.id,
                direction: UtilityDirection::Maximize,
                importance: 1.0,
            }],
            planning_horizon: 1,
            budgets: vec![],
            candidate_interventions: vec![],
            monte_carlo: MonteCarloConfig::new(1, 10, 20, 0.1, 0.1).unwrap(),
            scalar_preferences: None,
        },
    )
    .unwrap();
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project_document(0)),
            vec![entity_document(factor, 0)],
            vec![SourceDocument::new(
                "scenarios/A-plan.md",
                ScenarioDocument {
                    schema_version: SCHEMA_VERSION,
                    base_project_revision: 0,
                    scenario,
                },
            )],
        ),
        Err(ImportError::InvalidScenarioReference { .. })
    ));
}

fn import_with_node(project: ProjectDocument, node: Node) -> ValidatedImport {
    let revision = project.project.revision;
    ValidatedImport::new(
        SourceDocument::new("_project.md", project),
        vec![entity_document(node, revision)],
        vec![],
    )
    .unwrap()
}

#[test]
fn merge_plan_distinguishes_unchanged_create_and_update() {
    let current = import_with_node(project_document(2), node(0, "delivery"));
    let mut unchanged_from_stale_base = current.clone();
    unchanged_from_stale_base.project.document.project.revision = 1;
    unchanged_from_stale_base
        .entities
        .get_mut(&EntityId::new(0))
        .unwrap()
        .document
        .base_project_revision = 1;
    let plan = MergePlan::between(&current, &unchanged_from_stale_base);
    assert_eq!(plan.project, MergeAction::Unchanged);
    assert_eq!(plan.entities[&EntityId::new(0)], MergeAction::Unchanged);
    assert!(!plan.has_conflicts());

    let mut imported = current.clone();
    imported.project.document.description = "# Revised scope\n".to_owned();
    imported
        .entities
        .get_mut(&EntityId::new(0))
        .unwrap()
        .document
        .node
        .title = "Delivery flow".to_owned();
    imported
        .entities
        .insert(EntityId::new(1), entity_document(node(1, "quality"), 2));
    let plan = MergePlan::between(&current, &imported);
    assert_eq!(plan.project, MergeAction::Update);
    assert_eq!(plan.entities[&EntityId::new(0)], MergeAction::Update);
    assert_eq!(plan.entities[&EntityId::new(1)], MergeAction::Create);
    assert!(!plan.has_conflicts());
}

#[test]
fn merge_plan_conflicts_on_concurrent_or_cross_project_changes() {
    let current = import_with_node(project_document(2), node(0, "delivery"));
    let mut stale = current.clone();
    stale.project.document.project.revision = 1;
    let stale_node = &mut stale.entities.get_mut(&EntityId::new(0)).unwrap().document;
    stale_node.base_project_revision = 1;
    stale_node.node.title = "Stale edit".to_owned();
    assert!(matches!(
        MergePlan::between(&current, &stale).entities[&EntityId::new(0)],
        MergeAction::Conflict(MergeConflict::BaseRevision { .. })
    ));

    let mut aggregate_conflict = current.clone();
    let imported = &mut aggregate_conflict
        .entities
        .get_mut(&EntityId::new(0))
        .unwrap()
        .document
        .node;
    imported.revision = 1;
    imported.title = "Concurrent edit".to_owned();
    assert!(matches!(
        MergePlan::between(&current, &aggregate_conflict).entities[&EntityId::new(0)],
        MergeAction::Conflict(MergeConflict::AggregateRevision { .. })
    ));

    let mut foreign = current.clone();
    foreign.project.document.project.id = crate::domain::ProjectId::new("B").unwrap();
    assert!(matches!(
        MergePlan::between(&current, &foreign).project,
        MergeAction::Conflict(MergeConflict::DifferentProject { .. })
    ));
}

#[test]
fn rejects_dependence_addresses_without_an_imported_estimate() {
    let project_id = crate::domain::ProjectId::new("A").unwrap();
    let mut project = project_document(0);
    project.dependence = Some(ProjectDependenceModel {
        revision: 0,
        residual_groups: vec![ResidualDependenceGroup {
            members: vec![
                EstimateAddress::new(
                    project_id.clone(),
                    EstimateOwner::Node(EntityId::new(0)),
                    EstimateId::new(0),
                ),
                EstimateAddress::new(
                    project_id,
                    EstimateOwner::Node(EntityId::new(1)),
                    EstimateId::new(0),
                ),
            ],
            correlation: GaussianCopulaCorrelation {
                scale: CorrelationScale::Latent,
                matrix: vec![vec![1.0, 0.5], vec![0.5, 1.0]],
            },
        }],
    });
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project),
            vec![
                entity_document(node(0, "delivery"), 0),
                entity_document(node(1, "quality"), 0),
            ],
            vec![],
        ),
        Err(ImportError::MissingDependenceEstimate { .. })
    ));
}

#[test]
fn directory_round_trip_is_byte_stable_and_removes_stale_files() {
    let import = import_with_node(project_document(2), node(0, "delivery"));
    let expected = RenderedSnapshot::from_import(&import).unwrap();
    let root = std::env::temp_dir().join(format!("optimist-markdown-{}", uuid::Uuid::new_v4()));
    write_directory(&root, &expected).unwrap();
    let loaded = read_directory(&root).unwrap();
    assert_eq!(RenderedSnapshot::from_import(&loaded).unwrap(), expected);

    let stale = root.join("entities/stale.md");
    std::fs::write(&stale, "stale").unwrap();
    write_directory(&root, &expected).unwrap();
    assert!(!stale.exists());
    for (relative, content) in expected.files() {
        assert_eq!(
            std::fs::read_to_string(root.join(relative)).unwrap(),
            content
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_formulas_round_trip_and_validate_against_imported_primitives() {
    let project_id = crate::domain::ProjectId::new("A").unwrap();
    let root = EstimateAddress::new(
        project_id,
        EstimateOwner::Node(EntityId::new(0)),
        EstimateId::new(0),
    );
    let component = root
        .clone()
        .with_component(crate::domain::EstimateComponentId::new("baseline").unwrap());
    let mut factor = node(0, "delivery");
    factor.native_state = Some(
        crate::domain::QuantityState::new(
            crate::domain::QuantityDefinition::with_dimension(
                "state",
                Some(crate::domain::Unit::dimensionless()),
                None,
                crate::domain::QuantitySupport::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
            )
            .unwrap(),
            Some(
                crate::domain::Estimate::<crate::domain::QuantityValue>::new(
                    EstimateId::new(0),
                    crate::domain::Distribution::beta(2.0, 2.0).unwrap(),
                )
                .unwrap(),
            ),
            None,
        )
        .unwrap(),
    );
    let mut project = project_document(0);
    project.formulas = crate::domain::FormulaDocument {
        revision: 1,
        formulas: std::collections::BTreeMap::from([(
            component.clone(),
            crate::domain::Formula::Reference {
                address: root.clone(),
            },
        )]),
        provenance: std::collections::BTreeMap::from([(
            component,
            vec!["decomposition".to_owned()],
        )]),
    };
    let rendered = render_project(&project).unwrap();
    assert_eq!(parse_project("_project.md", &rendered).unwrap(), project);
    ValidatedImport::new(
        SourceDocument::new("_project.md", project),
        vec![entity_document(factor, 0)],
        vec![],
    )
    .unwrap();
}

#[test]
fn import_rejects_formula_roots_and_provenance_without_definitions() {
    let mut project = project_document(0);
    let root = EstimateAddress::new(
        project.project.id.clone(),
        EstimateOwner::Node(EntityId::new(0)),
        EstimateId::new(0),
    );
    let component = root
        .clone()
        .with_component(crate::domain::EstimateComponentId::new("missing").unwrap());
    project.formulas.formulas.insert(
        component,
        crate::domain::Formula::Literal {
            distribution: crate::domain::Distribution::point(1.0).unwrap(),
            unit: crate::domain::Unit::dimensionless(),
        },
    );
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project),
            vec![entity_document(node(0, "delivery"), 0)],
            vec![],
        ),
        Err(ImportError::InvalidFormulas { .. })
    ));

    let mut project = project_document(0);
    project
        .formulas
        .provenance
        .insert(root, vec!["orphan".to_owned()]);
    assert!(matches!(
        ValidatedImport::new(
            SourceDocument::new("_project.md", project),
            vec![entity_document(node(0, "delivery"), 0)],
            vec![],
        ),
        Err(ImportError::OrphanFormulaProvenance { .. })
    ));
}
