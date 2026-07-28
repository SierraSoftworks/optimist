//! The on-disk representation of a system design.
//!
//! # Layout
//!
//! A design is a directory rather than a file:
//!
//! ```text
//! _system.yaml              project metadata, shared quantities, scale units, interventions
//! components/<id>.yaml      one component and the relationships leaving it
//! component-types/<id>.yaml component types this project defines for itself
//! mutators/<id>.yaml        behaviours this project defines for itself
//! ```
//!
//! Splitting components across files is what makes a design reviewable. A model
//! large enough to be worth building is too large to read as one document, and
//! two engineers changing different parts of it should not meet in a diff. A
//! relationship is stored with the component it leaves, so adding a dependency
//! touches one file rather than a shared list whose ordering everyone would have
//! to agree on.
//!
//! Quantities that a design refers to as a whole stay together in `_system.yaml`.
//! Shared quantities, scale unit membership, and interventions are all statements
//! about the design rather than about a part of it, and scattering them would
//! mean reading every file to learn what a model assumes.
//!
//! # Project-local definitions
//!
//! A project may define its own component types and behaviours, loaded from the
//! directory alongside the shipped catalogue and checked by identical rules. A
//! design that needs a part nobody anticipated should not have to wait for the
//! catalogue to grow, and nothing in the shipped set is privileged.
//!
//! # Rejecting rather than repairing
//!
//! Unknown fields and unrecognised schema versions are refused. A file that
//! nearly parses is more dangerous than one that does not: silently dropping a
//! misspelt property would leave a model quietly using a default while its author
//! believed otherwise, and every number downstream would look plausible.
//!
//! This holds at every depth, not only at the top of a document. An author
//! writes inside the entries of a list far more often than beside the field
//! naming it, so a rule that guarded `scratchpad` while accepting anything
//! within one of its entries would protect the part nobody mistypes. It holds
//! for component type and behaviour manifests on the same reasoning, and there
//! it matters most: a manifest naming a section the engine has since renamed
//! produces a type with that section missing, which loads, solves, and reports
//! plausible numbers that are wrong wherever the section would have carried a
//! flow.

mod read;
mod write;

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

pub use read::{LoadedSystem, read_system};
pub use write::write_system;

use super::{
    intervention::Intervention,
    model::{Component, ComponentId, ScratchpadEntry},
    mutator::AttachedMutator,
    scale_unit::ScaleUnit,
};

/// The schema version this build reads and writes.
///
/// Version one described the causal graph this tool was built around before it
/// became a system design tool. The two schemas share no structure, so a version
/// one directory is refused rather than converted.
pub const SCHEMA_VERSION: u32 = 2;

/// The design-wide document, stored as `_system.yaml`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SystemDocument {
    /// Schema version this directory was written against.
    pub schema_version: u32,
    /// Human-readable name for the design.
    pub name: String,
    /// What the design is for.
    #[serde(default)]
    pub summary: String,
    /// Quantities shared across the design, in evaluation order.
    #[serde(default)]
    pub scratchpad: Vec<ScratchpadEntry>,
    /// Boundaries within which components are replicated together.
    #[serde(default)]
    pub scale_units: Vec<ScaleUnit>,
    /// Proposed changes, expressed as rebindings of shared quantities.
    #[serde(default)]
    pub interventions: Vec<Intervention>,
}

/// One component and the relationships leaving it.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentDocument {
    /// The component itself.
    #[serde(flatten)]
    pub component: Component,
    /// Relationships this component publishes onto.
    #[serde(default)]
    pub outgoing: Vec<OutgoingRelationship>,
}

/// A relationship stored with the component it leaves.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutgoingRelationship {
    /// Outbound port on the owning component this relationship leaves by.
    ///
    /// Omitted when the type declares exactly one outbound port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    /// Component receiving the flow.
    pub to: ComponentId,
    /// Inbound port on the receiving component this relationship arrives at.
    ///
    /// Omitted when the type declares exactly one inbound port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
    /// Squiggle source for how many operations may wait on this wire.
    ///
    /// Omitted to accept the default network link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<String>,
    /// Behaviours applied to the flow, in the order they take effect.
    #[serde(default)]
    pub mutators: Vec<AttachedMutator>,
    /// What this connection represents.
    #[serde(default)]
    pub summary: String,
}

/// Why a design could not be read or written.
#[derive(Debug)]
pub enum SchemaError {
    /// The directory could not be read or written.
    Io {
        /// The path being accessed.
        path: String,
        /// What the filesystem reported.
        source: std::io::Error,
    },
    /// A document is not well-formed YAML or does not match the schema.
    Malformed {
        /// The document being parsed.
        path: String,
        /// What the parser reported.
        message: String,
    },
    /// A directory was written by a schema this build does not read.
    UnsupportedVersion {
        /// The version found.
        found: u32,
    },
    /// An identifier cannot be used as a file name.
    UnsafeIdentifier {
        /// The rejected identifier.
        value: String,
    },
    /// Two documents claim the same identifier.
    Duplicate {
        /// The contested identifier.
        value: String,
    },
    /// A relationship names a component the design does not contain.
    DanglingRelationship {
        /// The component publishing the relationship.
        from: String,
        /// The component it named.
        to: String,
    },
    /// A project-local definition was rejected.
    Definition {
        /// The document being loaded.
        path: String,
        /// Why it was rejected.
        message: String,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{path}: {source}"),
            Self::Malformed { path, message } => write!(formatter, "{path}: {message}"),
            Self::UnsupportedVersion { found } => write!(
                formatter,
                "this design uses schema version {found}, and this build reads version {SCHEMA_VERSION}"
            ),
            Self::UnsafeIdentifier { value } => write!(
                formatter,
                "'{value}' cannot name a file; use lower-case letters, digits, hyphens, and underscores"
            ),
            Self::Duplicate { value } => {
                write!(formatter, "'{value}' is defined more than once")
            }
            Self::DanglingRelationship { from, to } => write!(
                formatter,
                "component '{from}' publishes to '{to}', which the design does not contain"
            ),
            Self::Definition { path, message } => write!(formatter, "{path}: {message}"),
        }
    }
}

impl std::error::Error for SchemaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Returns the file stem an identifier may be stored under.
///
/// Identifiers reach the filesystem, so they are checked rather than escaped. A
/// name containing a separator or a parent reference would otherwise let a
/// design decide where its own files land, and a design is data rather than
/// something entitled to choose paths.
pub fn safe_identifier(id: &str) -> Result<&str, SchemaError> {
    let safe = !id.is_empty()
        && id.len() <= 128
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        });
    safe.then_some(id)
        .ok_or_else(|| SchemaError::UnsafeIdentifier {
            value: id.to_owned(),
        })
}

pub(super) use safe_identifier as file_stem;

/// Reports the first identifier claimed twice.
pub(super) fn duplicate<'a>(ids: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut seen = BTreeMap::new();
    for id in ids {
        if seen.insert(id, ()).is_some() {
            return Some(id.to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_identifiers_are_accepted() {
        assert_eq!(file_stem("api-gateway").expect("safe"), "api-gateway");
        assert_eq!(file_stem("shard_01").expect("safe"), "shard_01");
    }

    #[test]
    fn identifiers_cannot_escape_their_directory() {
        for value in ["..", "../etc", "a/b", "a\\b", "/absolute", "."] {
            assert!(file_stem(value).is_err(), "'{value}' should be refused");
        }
    }

    #[test]
    fn identifiers_cannot_be_empty_or_unbounded() {
        assert!(file_stem("").is_err());
        assert!(file_stem(&"a".repeat(129)).is_err());
    }

    #[test]
    fn upper_case_is_refused_so_names_survive_case_folding_filesystems() {
        assert!(file_stem("API").is_err());
    }

    #[test]
    fn duplicates_are_reported() {
        assert_eq!(duplicate(["a", "b", "a"]), Some("a".to_owned()));
        assert_eq!(duplicate(["a", "b"]), None);
    }
}
