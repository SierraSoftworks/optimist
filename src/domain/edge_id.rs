use std::str::FromStr;

use thiserror::Error;

use super::{EdgeId, EdgeKind, EntityId, IdError};

/// Errors returned when a canonical external edge ID cannot be parsed.
///
/// Edge IDs use `<source>-<kind-token>-<destination>`. Entity IDs exclude `-`,
/// while the parser splits at the first and last delimiters so the `part-of`
/// token remains unambiguous.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum EdgeIdError {
    /// The text does not contain all three edge identity components.
    #[error("edge identifiers must use <source>-<kind>-<destination>")]
    InvalidFormat,
    /// The source or destination entity ID is invalid or non-canonical.
    #[error(transparent)]
    InvalidEntityId(#[from] IdError),
    /// The relationship token is not one of [`EdgeKind::token`]'s stable values.
    #[error("unknown edge kind token {0:?}")]
    InvalidKind(String),
}

impl FromStr for EdgeId {
    type Err = EdgeIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (source, remainder) = value.split_once('-').ok_or(EdgeIdError::InvalidFormat)?;
        let (kind, destination) = remainder
            .rsplit_once('-')
            .ok_or(EdgeIdError::InvalidFormat)?;
        Ok(Self {
            source: EntityId::from_str(source)?,
            kind: edge_kind(kind)?,
            destination: EntityId::from_str(destination)?,
        })
    }
}

fn edge_kind(value: &str) -> Result<EdgeKind, EdgeIdError> {
    match value {
        "contrib" => Ok(EdgeKind::Contributes),
        "measures" => Ok(EdgeKind::Measures),
        "changes" => Ok(EdgeKind::Changes),
        "requires" => Ok(EdgeKind::Requires),
        "part-of" => Ok(EdgeKind::PartOf),
        "blocks" => Ok(EdgeKind::Blocks),
        "conflicts" => Ok(EdgeKind::ConflictsWith),
        "synergizes" => Ok(EdgeKind::SynergizesWith),
        _ => Err(EdgeIdError::InvalidKind(value.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::EdgeIdError;
    use crate::domain::{EdgeId, EdgeKind, EntityId};

    #[test]
    fn round_trips_every_edge_kind() {
        for kind in [
            EdgeKind::Contributes,
            EdgeKind::Measures,
            EdgeKind::Changes,
            EdgeKind::Requires,
            EdgeKind::PartOf,
            EdgeKind::Blocks,
            EdgeKind::ConflictsWith,
            EdgeKind::SynergizesWith,
        ] {
            let edge = EdgeId {
                source: EntityId::new(0),
                kind,
                destination: EntityId::new(1),
            };
            assert_eq!(EdgeId::from_str(&edge.to_string()), Ok(edge));
        }
    }

    #[test]
    fn rejects_unknown_or_incomplete_ids() {
        assert_eq!(EdgeId::from_str("A-B"), Err(EdgeIdError::InvalidFormat));
        assert_eq!(
            EdgeId::from_str("A-unknown-B"),
            Err(EdgeIdError::InvalidKind("unknown".to_owned()))
        );
    }
}
