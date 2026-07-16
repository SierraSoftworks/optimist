use serde::de::DeserializeOwned;

use super::{
    MarkdownError,
    frontmatter::{EntityHeader, ProjectHeader, ScenarioDocumentHeader},
    model::{EntityDocument, ProjectDocument, SCHEMA_VERSION, ScenarioDocument},
    validate,
};

const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
const MAX_FRONTMATTER_BYTES: usize = 256 * 1024;

/// Parses a bounded `_project.md` string with source-aware YAML diagnostics.
pub fn parse_project(
    path: impl Into<String>,
    input: &str,
) -> Result<ProjectDocument, MarkdownError> {
    let path = path.into();
    let (frontmatter, body) = split(&path, input)?;
    let header: ProjectHeader = decode_yaml(&path, frontmatter)?;
    schema(&path, header.schema_version)?;
    Ok(ProjectDocument {
        schema_version: header.schema_version,
        project: header.project,
        description: body.to_owned(),
    })
}

/// Parses and validates a bounded entity Markdown string.
///
/// This validates document-local ownership and edge semantics. Cross-file destination
/// existence is intentionally deferred to the later two-pass import planner.
pub fn parse_entity(path: impl Into<String>, input: &str) -> Result<EntityDocument, MarkdownError> {
    let path = path.into();
    let (frontmatter, body) = split(&path, input)?;
    let header: EntityHeader = decode_yaml(&path, frontmatter)?;
    schema(&path, header.schema_version)?;
    let mut document = EntityDocument {
        schema_version: header.schema_version,
        base_project_revision: header.base_project_revision,
        node: header.node.into_node(body.to_owned()),
        outgoing_edges: header.outgoing_edges,
    };
    validate::entity(&path, &mut document)?;
    Ok(document)
}

/// Parses and aggregate-validates a bounded scenario Markdown document.
///
/// Outcome and intervention references are intentionally resolved later by the
/// project import layer, after all entity documents have been loaded.
pub fn parse_scenario(
    path: impl Into<String>,
    input: &str,
) -> Result<ScenarioDocument, MarkdownError> {
    let path = path.into();
    let (frontmatter, body) = split(&path, input)?;
    let header: ScenarioDocumentHeader = decode_yaml(&path, frontmatter)?;
    schema(&path, header.schema_version)?;
    let scenario = header.scenario.into_scenario(body.to_owned());
    scenario
        .draft
        .validate()
        .map_err(|error| MarkdownError::InvalidScenario {
            path: path.clone(),
            scenario: scenario.id,
            message: error.to_string(),
        })?;
    Ok(ScenarioDocument {
        schema_version: header.schema_version,
        base_project_revision: header.base_project_revision,
        scenario,
    })
}

fn split<'a>(path: &str, input: &'a str) -> Result<(&'a str, &'a str), MarkdownError> {
    if input.len() > MAX_DOCUMENT_BYTES {
        return Err(MarkdownError::DocumentTooLarge {
            path: path.to_owned(),
            maximum: MAX_DOCUMENT_BYTES,
        });
    }
    if input.contains('\r') {
        return Err(MarkdownError::NonCanonicalLineEndings(path.to_owned()));
    }
    let rest = input
        .strip_prefix("---\n")
        .ok_or_else(|| MarkdownError::MissingFrontmatter(path.to_owned()))?;
    let (yaml, body) = rest
        .split_once("\n---\n")
        .ok_or_else(|| MarkdownError::MissingFrontmatter(path.to_owned()))?;
    if yaml.len() > MAX_FRONTMATTER_BYTES {
        return Err(MarkdownError::FrontmatterTooLarge {
            path: path.to_owned(),
            maximum: MAX_FRONTMATTER_BYTES,
        });
    }
    Ok((yaml, body))
}

fn decode_yaml<T: DeserializeOwned>(path: &str, input: &str) -> Result<T, MarkdownError> {
    serde_yaml_ng::from_str(input).map_err(|error| {
        let location = error.location();
        MarkdownError::InvalidYaml {
            path: path.to_owned(),
            line: location.as_ref().map_or(2, |value| value.line() + 1),
            column: location.as_ref().map_or(1, |value| value.column()),
            message: error.to_string(),
        }
    })
}

fn schema(path: &str, version: u32) -> Result<(), MarkdownError> {
    if version != SCHEMA_VERSION {
        return Err(MarkdownError::UnsupportedSchema {
            path: path.to_owned(),
            version,
        });
    }
    Ok(())
}
