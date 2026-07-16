use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{EdgeId, EntityId};

/// Complete revision-checked replacement of node presentation metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UpdateNodeMetadata {
    /// Project-local node identity.
    pub id: EntityId,
    /// Node revision observed by the caller.
    pub expected_revision: u64,
    /// Nonempty human-facing display title.
    pub title: String,
    /// Rich Markdown explanation of meaning and boundaries.
    pub description: String,
    /// Complete replacement for extensible non-structural JSON metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Complete revision-checked replacement of edge presentation metadata.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UpdateEdgeMetadata {
    /// Canonical structural edge identity.
    pub id: EdgeId,
    /// Edge revision observed by the caller.
    pub expected_revision: u64,
    /// Rich Markdown explanation applying to the complete relationship.
    pub description: String,
    /// Complete replacement for extensible non-structural JSON metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}
