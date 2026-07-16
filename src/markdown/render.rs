use super::{
    MarkdownError,
    frontmatter::{EntityHeader, NodeHeader, ProjectHeader},
    model::{EntityDocument, ProjectDocument, SCHEMA_VERSION},
    validate,
};

/// Renders `_project.md` bytes with deterministic field order and LF endings.
pub fn render_project(document: &ProjectDocument) -> Result<String, MarkdownError> {
    schema(document.schema_version)?;
    let header = ProjectHeader {
        schema_version: document.schema_version,
        project: document.project.clone(),
    };
    render(&header, &document.description)
}

/// Validates and renders entity Markdown with outgoing edges in canonical ID order.
pub fn render_entity(document: &EntityDocument) -> Result<String, MarkdownError> {
    schema(document.schema_version)?;
    let mut document = document.clone();
    validate::entity("<render>", &mut document)?;
    let header = EntityHeader {
        schema_version: document.schema_version,
        base_project_revision: document.base_project_revision,
        node: NodeHeader::from_node(&document.node),
        outgoing_edges: document.outgoing_edges,
    };
    render(&header, &document.node.description)
}

fn schema(version: u32) -> Result<(), MarkdownError> {
    if version != SCHEMA_VERSION {
        return Err(MarkdownError::UnsupportedSchema {
            path: "<render>".to_owned(),
            version,
        });
    }
    Ok(())
}

fn render<T: serde::Serialize>(header: &T, body: &str) -> Result<String, MarkdownError> {
    if body.contains('\r') {
        return Err(MarkdownError::NonCanonicalLineEndings(
            "<render>".to_owned(),
        ));
    }
    let mut yaml = serde_yaml_ng::to_string(header)
        .map_err(|error| MarkdownError::Render(error.to_string()))?;
    if let Some(rest) = yaml.strip_prefix("---\n") {
        yaml = rest.to_owned();
    }
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    Ok(format!("---\n{}---\n{}", yaml, body))
}
