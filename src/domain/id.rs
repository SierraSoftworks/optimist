use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;
use uuid::Uuid;

const ID_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.";
const ENTITY_NAMESPACE: Uuid = Uuid::from_bytes([
    0x73, 0x69, 0x65, 0x72, 0x72, 0x61, 0x45, 0x40, 0x80, 0x00, 0x6f, 0x70, 0x74, 0x69, 0x6d, 0x69,
]);

/// Errors returned when external project or entity identifiers are not canonical.
///
/// Rejecting alternate spellings matters because identifiers are used in URLs,
/// Markdown references, edge IDs, and agent commands. A single canonical form
/// ensures those representations compare byte-for-byte.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdError {
    /// No identifier text was provided.
    #[error("an identifier cannot be empty")]
    Empty,
    /// The text contains a character outside Optimist's delimiter-safe alphabet.
    #[error("identifier contains an invalid character: {0:?}")]
    InvalidCharacter(char),
    /// The text decodes, but is not the shortest unique representation.
    #[error("identifier is not in its canonical form")]
    NonCanonical,
    /// The text represents a value larger than the underlying 64-bit counter.
    #[error("identifier exceeds the supported 64-bit range")]
    Overflow,
    /// A project ID is empty, too long, or contains a non-URL-safe character.
    #[error("project identifiers must be 1-64 URL-safe characters")]
    InvalidProjectId,
}

/// Identifies one isolated graph within an Optimist server.
///
/// Project IDs scope entity IDs, names, constraints, and analysis state. The same
/// entity ID may safely occur in two projects because callers always select a
/// project before addressing its graph.
///
/// ```
/// use optimist::domain::ProjectId;
///
/// let project = ProjectId::new("platform_reliability")?;
/// assert_eq!(project.as_str(), "platform_reliability");
/// # Ok::<(), optimist::domain::IdError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
    /// Validates and constructs a project ID suitable for URLs and filesystem keys.
    ///
    /// IDs may contain ASCII letters, digits, `_`, and `.`, and are limited to 64
    /// bytes. Human-facing project names are separate and may contain Unicode.
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
        {
            return Err(IdError::InvalidProjectId);
        }

        Ok(Self(value))
    }

    /// Returns the canonical text used in API paths and storage directories.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ProjectId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// A compact project-local identifier for a graph entity.
///
/// The underlying monotonic counter is rendered with a delimiter-safe base-64
/// alphabet, making IDs inexpensive for token-limited agents. Use [`Display`](fmt::Display)
/// for external representations and [`EntityId::to_indradb_uuid`] for storage.
///
/// ```
/// use optimist::domain::EntityId;
///
/// let id = EntityId::new(1);
/// assert_eq!(id.to_string(), "B");
/// assert_eq!(id, "B".parse()?);
/// # Ok::<(), optimist::domain::IdError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(u64);

impl EntityId {
    /// Wraps a value allocated by a project's monotonic entity counter.
    ///
    /// This constructor does not reserve the value; repositories own allocation
    /// and uniqueness. It is primarily useful when rebuilding IDs from storage.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the counter value used for ordering and allocating the next ID.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Maps this short ID deterministically to IndraDB's UUID key space.
    ///
    /// The UUID is version 5 and stable across restarts. Project stores are
    /// physically isolated, so equal entity counters in different projects may
    /// intentionally map to equal UUID bytes without colliding.
    ///
    /// ```
    /// use optimist::domain::EntityId;
    ///
    /// assert_eq!(
    ///     EntityId::new(42).to_indradb_uuid(),
    ///     EntityId::new(42).to_indradb_uuid()
    /// );
    /// ```
    pub fn to_indradb_uuid(self) -> Uuid {
        Uuid::new_v5(&ENTITY_NAMESPACE, &self.0.to_be_bytes())
    }

    fn encode(self) -> String {
        if self.0 == 0 {
            return "A".to_owned();
        }

        let mut value = self.0;
        let mut buffer = [0_u8; 11];
        let mut cursor = buffer.len();
        while value > 0 {
            cursor -= 1;
            buffer[cursor] = ID_ALPHABET[(value & 0x3f) as usize];
            value >>= 6;
        }

        String::from_utf8(buffer[cursor..].to_vec()).expect("identifier alphabet is ASCII")
    }

    fn decode(value: &str) -> Result<Self, IdError> {
        if value.is_empty() {
            return Err(IdError::Empty);
        }
        if value.len() > 11 {
            return Err(IdError::NonCanonical);
        }

        let mut decoded = 0_u64;
        for character in value.chars() {
            let digit = ID_ALPHABET
                .iter()
                .position(|candidate| *candidate == character as u8)
                .ok_or(IdError::InvalidCharacter(character))? as u64;
            decoded = decoded
                .checked_mul(64)
                .and_then(|current| current.checked_add(digit))
                .ok_or(IdError::Overflow)?;
        }

        let id = Self(decoded);
        if id.encode() != value {
            return Err(IdError::NonCanonical);
        }
        Ok(id)
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.encode())
    }
}

impl FromStr for EntityId {
    type Err = IdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::decode(value)
    }
}

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::decode(&value).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use proptest::prelude::*;

    use super::{EntityId, IdError, ProjectId};

    #[test]
    fn rejects_non_canonical_and_delimited_ids() {
        assert_eq!(EntityId::from_str("AA"), Err(IdError::NonCanonical));
        assert_eq!(
            EntityId::from_str("A-B"),
            Err(IdError::InvalidCharacter('-'))
        );
    }

    #[test]
    fn validates_project_ids() {
        assert!(ProjectId::new("platform.reliability_2026").is_ok());
        assert_eq!(
            ProjectId::new("another/project"),
            Err(IdError::InvalidProjectId)
        );
    }

    proptest! {
        #[test]
        fn entity_ids_round_trip(value in any::<u64>()) {
            let id = EntityId::new(value);
            prop_assert_eq!(EntityId::from_str(&id.to_string()), Ok(id));
        }

        #[test]
        fn entity_ids_round_trip_through_json(value in any::<u64>()) {
            let id = EntityId::new(value);
            let encoded = serde_json::to_string(&id).expect("serialize entity ID");
            let decoded: EntityId = serde_json::from_str(&encoded).expect("deserialize entity ID");
            prop_assert_eq!(decoded, id);
        }

        #[test]
        fn internal_uuids_are_deterministic(value in any::<u64>()) {
            let id = EntityId::new(value);
            prop_assert_eq!(id.to_indradb_uuid(), id.to_indradb_uuid());
        }
    }
}
