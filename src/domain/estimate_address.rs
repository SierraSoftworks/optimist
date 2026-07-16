use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use super::{EdgeId, EdgeIdError, EntityId, EstimateId, IdError, ProjectId};

/// Errors returned when an embedded estimate address is invalid.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EstimateAddressError {
    /// The address does not follow the canonical tagged path grammar.
    #[error("estimate addresses must identify a project, owner, estimate, and optional components")]
    InvalidFormat,
    /// A project, node, or estimate identifier is invalid.
    #[error(transparent)]
    InvalidId(#[from] IdError),
    /// The canonical edge identifier is invalid.
    #[error(transparent)]
    InvalidEdgeId(#[from] EdgeIdError),
    /// A nested Fermi component identifier is empty or not delimiter-safe.
    #[error("invalid estimate component identifier {0:?}")]
    InvalidComponentId(String),
}

/// Identifies the graph aggregate which embeds an estimate.
///
/// This enum deliberately names only current storage owners. It does not imply
/// that every node or edge payload exposes estimates through a generic accessor.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum EstimateOwner {
    /// A project-local graph node.
    Node(EntityId),
    /// A canonical directed or symmetric graph edge.
    Edge(EdgeId),
}

/// A stable identifier for a nested component of an embedded Fermi estimate.
///
/// Component IDs are aggregate-local and have no meaning without the complete
/// [`EstimateAddress`]. They use a URL-path-safe grammar so text addresses need
/// no escaping.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EstimateComponentId(String);

impl EstimateComponentId {
    /// Validates a stable component identifier.
    ///
    /// IDs must start with an ASCII letter, contain only ASCII letters, digits,
    /// `_`, `.`, or `-`, and be at most 64 bytes long.
    pub fn new(value: impl Into<String>) -> Result<Self, EstimateAddressError> {
        let value = value.into();
        let mut bytes = value.bytes();
        if value.len() > 64
            || !bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic())
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
        {
            return Err(EstimateAddressError::InvalidComponentId(value));
        }
        Ok(Self(value))
    }

    /// Returns the canonical component identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for EstimateComponentId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A stable, project-scoped address for an estimate embedded in a node or edge.
///
/// The required estimate ID is local to its owner. Additional component IDs form
/// an ordered path into future nested Fermi structures without inventing storage
/// ownership for payloads that do not currently contain estimates. The canonical
/// text form is `<project>/<node|edge>/<owner>/estimate/<id>` followed by zero or
/// more `/component/<id>` pairs.
///
/// ```
/// use optimist::domain::{EntityId, EstimateAddress, EstimateId, EstimateOwner, ProjectId};
///
/// let address = EstimateAddress::new(
///     ProjectId::new("forecast")?,
///     EstimateOwner::Node(EntityId::new(1)),
///     EstimateId::new(2),
/// );
/// let text = address.to_string();
/// assert_eq!(text.parse::<EstimateAddress>()?, address);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EstimateAddress {
    /// Project whose isolated graph contains the owner.
    pub project: ProjectId,
    /// Node or canonical edge embedding the estimate.
    pub owner: EstimateOwner,
    /// Owner-local identity of the embedded estimate.
    pub estimate: EstimateId,
    /// Ordered aggregate-local path to a nested Fermi component.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<EstimateComponentId>,
}

impl EstimateAddress {
    /// Constructs an address for an owner-local estimate root.
    pub fn new(project: ProjectId, owner: EstimateOwner, estimate: EstimateId) -> Self {
        Self {
            project,
            owner,
            estimate,
            components: Vec::new(),
        }
    }

    /// Appends a validated nested component to this address.
    pub fn with_component(mut self, component: EstimateComponentId) -> Self {
        self.components.push(component);
        self
    }
}

impl fmt::Display for EstimateAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/", self.project)?;
        match &self.owner {
            EstimateOwner::Node(id) => write!(formatter, "node/{id}")?,
            EstimateOwner::Edge(id) => write!(formatter, "edge/{id}")?,
        }
        write!(formatter, "/estimate/{}", self.estimate)?;
        for component in &self.components {
            write!(formatter, "/component/{}", component.as_str())?;
        }
        Ok(())
    }
}

impl FromStr for EstimateAddress {
    type Err = EstimateAddressError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('/');
        let project = ProjectId::from_str(parts.next().ok_or(Self::Err::InvalidFormat)?)?;
        let owner_kind = parts.next().ok_or(Self::Err::InvalidFormat)?;
        let owner_id = parts.next().ok_or(Self::Err::InvalidFormat)?;
        let owner = match owner_kind {
            "node" => EstimateOwner::Node(EntityId::from_str(owner_id)?),
            "edge" => EstimateOwner::Edge(EdgeId::from_str(owner_id)?),
            _ => return Err(Self::Err::InvalidFormat),
        };
        if parts.next() != Some("estimate") {
            return Err(Self::Err::InvalidFormat);
        }
        let estimate = EntityId::from_str(parts.next().ok_or(Self::Err::InvalidFormat)?)?;
        let mut address = Self::new(project, owner, EstimateId::new(estimate.value()));
        while let Some(marker) = parts.next() {
            if marker != "component" {
                return Err(Self::Err::InvalidFormat);
            }
            let component = parts.next().ok_or(Self::Err::InvalidFormat)?;
            address
                .components
                .push(EstimateComponentId::new(component)?);
        }
        Ok(address)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{EstimateAddress, EstimateAddressError, EstimateComponentId, EstimateOwner};
    use crate::domain::{EdgeId, EdgeKind, EntityId, EstimateId, ProjectId};

    fn edge() -> EdgeId {
        EdgeId {
            source: EntityId::new(1),
            kind: EdgeKind::PartOf,
            destination: EntityId::new(2),
        }
    }

    #[test]
    fn rejects_empty_or_ambiguous_components() {
        for value in ["", "1first", "two/parts"] {
            assert!(matches!(
                EstimateComponentId::new(value),
                Err(EstimateAddressError::InvalidComponentId(_))
            ));
        }
        assert!(
            "project/node/A/estimate/B/component"
                .parse::<EstimateAddress>()
                .is_err()
        );
    }

    #[test]
    fn edge_addresses_are_canonical() {
        let address = EstimateAddress::new(
            ProjectId::new("forecast").unwrap(),
            EstimateOwner::Edge(edge()),
            EstimateId::new(3),
        )
        .with_component(EstimateComponentId::new("labor-hours").unwrap());
        assert_eq!(
            address.to_string(),
            "forecast/edge/B-part-of-C/estimate/D/component/labor-hours"
        );
        assert_eq!(address.to_string().parse(), Ok(address));
    }

    proptest! {
        #[test]
        fn node_addresses_round_trip_through_text_and_json(owner in any::<u64>(), estimate in any::<u64>()) {
            let address = EstimateAddress::new(
                ProjectId::new("project_1").unwrap(),
                EstimateOwner::Node(EntityId::new(owner)),
                EstimateId::new(estimate),
            )
            .with_component(EstimateComponentId::new("component.1").unwrap());
            prop_assert_eq!(address.to_string().parse::<EstimateAddress>().unwrap(), address.clone());
            let json = serde_json::to_string(&address).unwrap();
            prop_assert_eq!(serde_json::from_str::<EstimateAddress>(&json).unwrap(), address);
        }
    }
}
