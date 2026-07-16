use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{EntityId, IdError};

/// Structural relationship kinds supported by the causal graph.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// A signed causal effect flowing from a factor/outcome to another subject.
    Contributes,
    /// A metric observing a specific factor or outcome.
    Measures,
    /// An intervention's expected signed effect on a factor.
    Changes,
    /// A hard or soft prerequisite from a factor/intervention to another.
    Requires,
    /// Non-causal decomposition of a factor into a parent factor.
    PartOf,
    /// A factor preventing or reducing another factor/intervention.
    Blocks,
    /// Symmetric incompatibility between intervention choices.
    ConflictsWith,
    /// Symmetric beneficial interaction between intervention choices.
    SynergizesWith,
}

impl EdgeKind {
    /// Returns the stable delimiter-safe token used in IndraDB and external IDs.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Contributes => "contrib",
            Self::Measures => "measures",
            Self::Changes => "changes",
            Self::Requires => "requires",
            Self::PartOf => "part-of",
            Self::Blocks => "blocks",
            Self::ConflictsWith => "conflicts",
            Self::SynergizesWith => "synergizes",
        }
    }

    pub(super) const fn is_symmetric(self) -> bool {
        matches!(self, Self::ConflictsWith | Self::SynergizesWith)
    }
}

/// Canonical identity of an edge within one project.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EdgeId {
    /// Outbound project-local entity.
    pub source: EntityId,
    /// Semantic relationship kind.
    pub kind: EdgeKind,
    /// Inbound project-local entity.
    pub destination: EntityId,
}

impl fmt::Display for EdgeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}-{}-{}",
            self.source,
            self.kind.token(),
            self.destination
        )
    }
}

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
