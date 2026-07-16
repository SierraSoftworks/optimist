use crate::{
    domain::{Edge, Node, Scenario, normalize_name},
    project::Project,
};

/// Markdown schema version currently accepted and emitted by Optimist.
pub const SCHEMA_VERSION: u32 = 1;

/// Versioned `_project.md` content independent of its filesystem location.
///
/// Frontmatter stores identity and revision metadata; `description` is the Markdown
/// body. Directory import will later combine this value with entity documents.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project metadata at the exported base revision.
    pub project: Project,
    /// Rich Markdown project rationale and scope.
    pub description: String,
}

/// Versioned entity Markdown content with all relationships owned by its source node.
///
/// The node description is stored only in `Node::description` and rendered as the
/// Markdown body. `outgoing_edges` belong in this file because their source identity
/// is structurally fixed to `node.id`.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project graph revision from which this entity file was exported.
    pub base_project_revision: u64,
    /// Complete typed node aggregate reconstructed with its Markdown description.
    pub node: Node,
    /// Complete outgoing edge aggregates, sorted by canonical edge ID when rendered.
    pub outgoing_edges: Vec<Edge>,
}

/// Versioned canonical `scenarios/<id>-<slug>.md` content.
///
/// Structured scenario fields live in YAML frontmatter while the scenario's
/// Markdown rationale is the document body. Entity references remain unresolved
/// until project-level import validation has loaded the complete graph.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project revision from which this scenario file was exported.
    pub base_project_revision: u64,
    /// Complete typed scenario reconstructed with its Markdown rationale.
    pub scenario: Scenario,
}

impl ScenarioDocument {
    /// Returns the deterministic relative export path for this document.
    pub fn canonical_path(&self) -> String {
        let normalized = normalize_name(&self.scenario.draft.name);
        let mut slug = String::new();
        let mut separator = false;
        for character in normalized.chars() {
            if character.is_alphanumeric() {
                if separator && !slug.is_empty() {
                    slug.push('-');
                }
                separator = false;
                slug.push(character);
            } else {
                separator = true;
            }
        }
        if slug.is_empty() {
            slug.push_str("scenario");
        }
        format!("scenarios/{}-{slug}.md", self.scenario.id)
    }
}
