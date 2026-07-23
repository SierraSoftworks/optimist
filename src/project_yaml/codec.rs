use std::collections::BTreeSet;

use serde::{Serialize, de::DeserializeOwned};

use crate::domain::{Edge, normalize_name};

use super::{EntityDocument, ProjectDocument, SCHEMA_VERSION, ScenarioDocument, YamlError};

const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

/// Parses and validates a bounded `_project.yaml` document.
pub fn parse_project(path: impl Into<String>, input: &str) -> Result<ProjectDocument, YamlError> {
    let path = path.into();
    let document: ProjectDocument = decode(&path, input)?;
    schema(&path, document.schema_version)?;
    if let Some(dependence) = &document.dependence {
        dependence
            .validate_for_project(&document.project.id)
            .map_err(|error| YamlError::InvalidDependence {
                path,
                message: error.to_string(),
            })?;
    }
    Ok(document)
}

/// Parses and validates a bounded YAML entity document.
pub fn parse_entity(path: impl Into<String>, input: &str) -> Result<EntityDocument, YamlError> {
    let path = path.into();
    let mut document: EntityDocument = decode(&path, input)?;
    schema(&path, document.schema_version)?;
    validate_entity(&path, &mut document)?;
    Ok(document)
}

/// Parses and validates a bounded YAML scenario document.
pub fn parse_scenario(path: impl Into<String>, input: &str) -> Result<ScenarioDocument, YamlError> {
    let path = path.into();
    let document: ScenarioDocument = decode(&path, input)?;
    schema(&path, document.schema_version)?;
    document
        .scenario
        .draft
        .validate()
        .map_err(|error| YamlError::InvalidScenario {
            path,
            scenario: document.scenario.id,
            message: error.to_string(),
        })?;
    Ok(document)
}

/// Validates and renders one canonical YAML project document.
pub fn render_project(document: &ProjectDocument) -> Result<String, YamlError> {
    schema("<render>", document.schema_version)?;
    if let Some(dependence) = &document.dependence {
        dependence
            .validate_for_project(&document.project.id)
            .map_err(|error| YamlError::InvalidDependence {
                path: "<render>".to_owned(),
                message: error.to_string(),
            })?;
    }
    render(document)
}

/// Validates and renders one canonical YAML entity document.
pub fn render_entity(document: &EntityDocument) -> Result<String, YamlError> {
    schema("<render>", document.schema_version)?;
    let mut document = document.clone();
    validate_entity("<render>", &mut document)?;
    render(&document)
}

/// Validates and renders one canonical YAML scenario document.
pub fn render_scenario(document: &ScenarioDocument) -> Result<String, YamlError> {
    schema("<render>", document.schema_version)?;
    document
        .scenario
        .draft
        .validate()
        .map_err(|error| YamlError::InvalidScenario {
            path: "<render>".to_owned(),
            scenario: document.scenario.id,
            message: error.to_string(),
        })?;
    render(document)
}

fn decode<T: DeserializeOwned>(path: &str, input: &str) -> Result<T, YamlError> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err(YamlError::DocumentTooLarge {
            path: path.to_owned(),
            maximum: MAX_DOCUMENT_BYTES,
        });
    }
    if input.contains('\r') {
        return Err(YamlError::NonCanonicalLineEndings(path.to_owned()));
    }
    serde_yaml_ng::from_str(input).map_err(|error| YamlError::InvalidYaml {
        path: path.to_owned(),
        message: error.to_string(),
    })
}

fn render<T: Serialize>(document: &T) -> Result<String, YamlError> {
    let mut yaml =
        serde_yaml_ng::to_string(document).map_err(|error| YamlError::Render(error.to_string()))?;
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    Ok(yaml)
}

fn schema(path: &str, version: u32) -> Result<(), YamlError> {
    if version != SCHEMA_VERSION {
        return Err(YamlError::UnsupportedSchema {
            path: path.to_owned(),
            version,
        });
    }
    Ok(())
}

fn validate_entity(path: &str, document: &mut EntityDocument) -> Result<(), YamlError> {
    document
        .node
        .validate_native_state()
        .map_err(|error| YamlError::InvalidNode {
            path: path.to_owned(),
            node: document.node.id,
            message: error.to_string(),
        })?;
    if document.node.normalized_name != normalize_name(&document.node.name) {
        return Err(YamlError::InvalidNodeName {
            path: path.to_owned(),
            node: document.node.id,
        });
    }
    let mut ids = BTreeSet::new();
    for edge in &document.outgoing_edges {
        let id = edge.id();
        if edge.source != document.node.id {
            return Err(YamlError::ForeignOutgoingEdge {
                path: path.to_owned(),
                node: document.node.id,
                edge: id,
            });
        }
        if !ids.insert(id.clone()) {
            return Err(YamlError::DuplicateEdge {
                path: path.to_owned(),
                edge: id,
            });
        }
        let validated = Edge::new(
            edge.source,
            edge.source_kind,
            edge.destination,
            edge.destination_kind,
            edge.payload.clone(),
        )
        .map_err(|error| YamlError::InvalidEdge {
            path: path.to_owned(),
            edge: id.clone(),
            message: error.to_string(),
        })?;
        if validated.id() != id {
            return Err(YamlError::InvalidEdge {
                path: path.to_owned(),
                edge: id,
                message: "edge identity is not canonical".to_owned(),
            });
        }
    }
    document.outgoing_edges.sort_by_key(Edge::id);
    Ok(())
}
