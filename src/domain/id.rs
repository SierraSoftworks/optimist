use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;
use uuid::Uuid;

const ID_ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.";
const ENTITY_NAMESPACE: Uuid = Uuid::from_bytes([
    0x73, 0x69, 0x65, 0x72, 0x72, 0x61, 0x45, 0x40, 0x80, 0x00, 0x6f, 0x70, 0x74, 0x69, 0x6d, 0x69,
]);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum IdError {
    #[error("an identifier cannot be empty")]
    Empty,
    #[error("identifier contains an invalid character: {0:?}")]
    InvalidCharacter(char),
    #[error("identifier is not in its canonical form")]
    NonCanonical,
    #[error("identifier exceeds the supported 64-bit range")]
    Overflow,
    #[error("project identifiers must be 1-64 URL-safe characters")]
    InvalidProjectId,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(String);

impl ProjectId {
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityId(u64);

impl EntityId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

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
