use std::collections::BTreeSet;

use crate::domain::{Edge, normalize_name};

use super::{EntityDocument, MarkdownError};

pub(super) fn entity(path: &str, document: &mut EntityDocument) -> Result<(), MarkdownError> {
    if document.node.normalized_name != normalize_name(&document.node.name) {
        return Err(MarkdownError::InvalidNodeName {
            path: path.to_owned(),
            node: document.node.id,
        });
    }
    let mut ids = BTreeSet::new();
    for edge in &document.outgoing_edges {
        let id = edge.id();
        if edge.source != document.node.id {
            return Err(MarkdownError::ForeignOutgoingEdge {
                path: path.to_owned(),
                node: document.node.id,
                edge: id,
            });
        }
        if !ids.insert(id.clone()) {
            return Err(MarkdownError::DuplicateEdge {
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
        .map_err(|error| MarkdownError::InvalidEdge {
            path: path.to_owned(),
            edge: id.clone(),
            message: error.to_string(),
        })?;
        if validated.id() != id {
            return Err(MarkdownError::InvalidEdge {
                path: path.to_owned(),
                edge: id,
                message: "edge identity is not canonical".to_owned(),
            });
        }
    }
    document.outgoing_edges.sort_by_key(Edge::id);
    Ok(())
}
