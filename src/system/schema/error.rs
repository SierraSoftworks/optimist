//! Why a design could not be read or written.

use std::fmt;

use super::SCHEMA_VERSION;

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
