use serde::{Deserialize, Serialize};

use crate::{
    domain::{Edge, Node, ProjectDependenceModel, Scenario, normalize_name},
    project::Project,
};

/// YAML project schema version currently accepted and emitted by Optimist.
pub const SCHEMA_VERSION: u32 = 1;

/// Versioned `_project.yaml` content independent of its filesystem location.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project metadata at the exported base revision.
    pub project: Project,
    /// Optional project-level Gaussian residual dependence document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependence: Option<ProjectDependenceModel>,
    /// Project rationale and scope.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Versioned YAML entity with all relationships owned by their source node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EntityDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project graph revision from which this entity was exported.
    pub base_project_revision: u64,
    /// Complete typed node aggregate.
    pub node: Node,
    /// Complete outgoing edge aggregates in canonical edge-ID order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outgoing_edges: Vec<Edge>,
}

impl EntityDocument {
    /// Returns the deterministic relative YAML export path for this entity.
    pub fn canonical_path(&self) -> String {
        format!(
            "entities/{}-{}.yaml",
            self.node.id,
            slug(&self.node.name, "entity")
        )
    }
}

/// Versioned `scenarios/<id>-<slug>.yaml` content.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDocument {
    /// Schema version used to decode this document.
    pub schema_version: u32,
    /// Project revision from which this scenario was exported.
    pub base_project_revision: u64,
    /// Complete typed scenario aggregate.
    pub scenario: Scenario,
}

impl ScenarioDocument {
    /// Returns the deterministic relative YAML export path for this scenario.
    pub fn canonical_path(&self) -> String {
        format!(
            "scenarios/{}-{}.yaml",
            self.scenario.id,
            slug(&self.scenario.draft.name, "scenario")
        )
    }
}

fn slug(value: &str, fallback: &str) -> String {
    let normalized = normalize_name(value);
    let mut result = String::new();
    let mut separator = false;
    for character in normalized.chars() {
        if character.is_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('_');
            }
            separator = false;
            result.push(character);
        } else {
            separator = true;
        }
    }
    if result.is_empty() {
        result.push_str(fallback);
    }
    result
}
