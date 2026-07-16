use std::collections::BTreeMap;

use crate::domain::{Edge, EdgeId, EntityId, Node};

use super::{RepositoryError, RepositoryResult};

pub(super) fn validated(nodes: &BTreeMap<EntityId, Node>, edge: Edge) -> RepositoryResult<Edge> {
    let source_kind = nodes
        .get(&edge.source)
        .ok_or(RepositoryError::MissingEntity(edge.source))?
        .kind();
    let destination_kind = nodes
        .get(&edge.destination)
        .ok_or(RepositoryError::MissingEntity(edge.destination))?
        .kind();
    for (id, actual, declared) in [
        (edge.source, source_kind, edge.source_kind),
        (edge.destination, destination_kind, edge.destination_kind),
    ] {
        if actual != declared {
            return Err(RepositoryError::EndpointKindMismatch {
                id,
                actual,
                declared,
            });
        }
    }

    let revision = edge.revision;
    let description = edge.description;
    let metadata = edge.metadata;
    let mut edge = Edge::new(
        edge.source,
        source_kind,
        edge.destination,
        destination_kind,
        edge.payload,
    )?;
    edge.revision = revision;
    edge.description = description;
    edge.metadata = metadata;
    Ok(edge)
}

pub(super) fn update(
    nodes: &BTreeMap<EntityId, Node>,
    edges: &mut BTreeMap<EdgeId, Edge>,
    edge: Edge,
) -> RepositoryResult<()> {
    let id = edge.id();
    if !edges.contains_key(&id) {
        return Err(RepositoryError::MissingEdge(id.to_string()));
    }
    edges.insert(id, validated(nodes, edge)?);
    Ok(())
}
