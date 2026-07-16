use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{EntityId, IdError};

/// Compact project-local identity for a scenario document.
///
/// Scenario IDs use the same stable short encoding as graph entities, but occupy
/// an independent namespace because scenarios are project documents, not vertices.
///
/// ```
/// use optimist::domain::ScenarioId;
///
/// let id = ScenarioId::new(1);
/// assert_eq!(id.to_string(), "B");
/// assert_eq!(id, "B".parse()?);
/// # Ok::<(), optimist::domain::IdError>(())
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ScenarioId(EntityId);

impl ScenarioId {
    /// Constructs an ID from the project-local scenario counter.
    pub const fn new(value: u64) -> Self {
        Self(EntityId::new(value))
    }

    /// Returns the project-local counter value used for deterministic ordering.
    pub const fn value(self) -> u64 {
        self.0.value()
    }
}

impl fmt::Display for ScenarioId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for ScenarioId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}
