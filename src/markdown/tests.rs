use crate::{
    domain::{
        Edge, EdgePayload, EntityId, Factor, MonteCarloConfig, Node, NodeKind, NodePayload,
        Scenario, ScenarioDraft, ScenarioId,
    },
    markdown::{
        EntityDocument, MarkdownError, ProjectDocument, SCHEMA_VERSION, ScenarioDocument,
        parse_entity, parse_project, parse_scenario, render_entity, render_project,
        render_scenario,
    },
    project::Project,
};

fn node(id: u64, name: &str) -> Node {
    let mut node = Node::new(
        EntityId::new(id),
        name,
        name,
        NodePayload::Factor(Factor {
            current: None,
            desired: None,
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
    let document = EntityDocument {
        schema_version: SCHEMA_VERSION,
        base_project_revision: 7,
        node: node(0, "delivery"),
        outgoing_edges: vec![edge(0, 2), edge(0, 1)],
    };
    let first = render_entity(&document).unwrap();
    let second = render_entity(&document).unwrap();
    assert_eq!(first, second);
    assert!(!first.contains('\r'));
    let parsed = parse_entity("entities/A-delivery.md", &first).unwrap();
    assert_eq!(parsed.outgoing_edges[0].destination, EntityId::new(1));
    assert_eq!(render_entity(&parsed).unwrap(), first);
}

#[test]
fn project_body_is_separate_from_frontmatter() {
    let document = ProjectDocument {
        schema_version: SCHEMA_VERSION,
        project: Project {
            id: crate::domain::ProjectId::new("A").unwrap(),
            name: "Delivery".to_owned(),
            revision: 3,
        },
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
