use std::collections::BTreeSet;

use crate::domain::{EntityId, Node, normalize_name};

use super::{RepositoryError, RepositoryResult};

pub(super) fn name_claims(node: &Node) -> RepositoryResult<Vec<String>> {
    let expected = normalize_name(&node.name);
    if node.normalized_name != expected {
        return Err(RepositoryError::InvalidNormalizedName {
            id: node.id,
            actual: node.normalized_name.clone(),
            expected,
        });
    }

    let claims = std::iter::once(node.normalized_name.clone())
        .chain(node.aliases.iter().map(|alias| normalize_name(alias)))
        .collect::<Vec<_>>();
    let unique = claims.iter().collect::<BTreeSet<_>>();
    if claims.iter().any(String::is_empty) || unique.len() != claims.len() {
        return Err(RepositoryError::InvalidNameClaim(node.id));
    }
    Ok(claims)
}

pub(super) fn validate_node_update(current: &Node, replacement: &Node) -> RepositoryResult<()> {
    if current.id != replacement.id
        || current.kind() != replacement.kind()
        || current.name != replacement.name
        || current.normalized_name != replacement.normalized_name
        || current.title != replacement.title
        || current.description != replacement.description
        || current.aliases != replacement.aliases
        || current.metadata != replacement.metadata
    {
        return Err(RepositoryError::NodeUpdateChangedMetadata(current.id));
    }
    Ok(())
}

pub(super) fn advance_entity_id(next: &mut Option<u64>, id: EntityId) {
    if next.is_some_and(|candidate| id.value() >= candidate) {
        *next = id.value().checked_add(1);
    }
}
