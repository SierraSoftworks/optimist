//! Moving a design between machines as a single file.
//!
//! # Why a zip of the directory rather than one document
//!
//! A design is a directory because that is what makes it reviewable, and
//! flattening it into one document for transport would mean maintaining a second
//! representation that has to agree with the first forever. A zip carries the
//! directory as it stands, so what an engineer receives is byte-for-byte what
//! their colleague had, and anybody without this tool can still open it.
//!
//! # Treating every archive as hostile
//!
//! An archive arrives by email, chat, or download, and nothing about it can be
//! trusted. Entry names may try to escape the directory they are unpacked into,
//! compressed bytes may expand to fill a disk, and the contents may be from a
//! version of this tool that wrote a schema this one does not read.
//!
//! Every one of those is refused before anything is written. Only the four paths
//! a design is made of are extracted, each entry is read through a ceiling
//! rather than to its declared size, and the result is loaded with the ordinary
//! reader before it is allowed to become a design. An archive that fails any of
//! those checks leaves nothing behind at all, which is why unpacking stages into
//! a scratch directory and moves it into place only once it has passed.

mod pack;
mod unpack;

use std::fmt;

pub use pack::pack_system;
pub use unpack::StagedDesign;

use super::SchemaError;

/// The largest archive that will be read at all, in bytes.
///
/// Refusing early costs nothing and bounds everything downstream. A design is
/// YAML written by people, so anything approaching this is either a mistake or
/// an attempt to exhaust the machine reading it.
pub const MAX_ARCHIVE_BYTES: u64 = 16 << 20;

/// The largest a single unpacked document may be, in bytes.
const MAX_ENTRY_BYTES: u64 = 4 << 20;

/// The largest a whole unpacked design may be, in bytes.
const MAX_UNPACKED_BYTES: u64 = 64 << 20;

/// The most entries an archive may declare.
const MAX_ENTRIES: usize = 4_096;

/// The document naming the design, and the only file required to be present.
const MANIFEST: &str = "_system.yaml";

/// The directories a design keeps its documents in, in reading order.
const DIRECTORIES: [&str; 3] = ["components", "component-types", "mutators"];

/// Why a design could not be packed or unpacked.
#[derive(Debug)]
pub enum ArchiveError {
    /// The file is not a readable zip archive.
    Unreadable {
        /// What the reader reported, kept for a log rather than for a person.
        ///
        /// Every message a zip reader produces at this point is about a
        /// structure nobody outside one has heard of, and putting it in front of
        /// somebody holding a truncated download tells them less than the advice
        /// beside it already does.
        message: String,
    },
    /// The archive holds no `_system.yaml`, so it is not a design.
    NotADesign,
    /// A document sits somewhere a design has no place for.
    Misplaced {
        /// The entry that was refused.
        entry: String,
    },
    /// The archive declares more entries than a design can plausibly have.
    TooManyEntries {
        /// The most that would have been read.
        limit: usize,
    },
    /// Unpacking the archive would produce more data than is plausible.
    TooLarge {
        /// The ceiling that was passed.
        limit: u64,
    },
    /// The archive unpacked, and what it contained is not a design this build reads.
    Invalid {
        /// Why the unpacked directory was refused.
        source: SchemaError,
    },
    /// The filesystem refused a read or a write.
    Io {
        /// The path being accessed.
        path: String,
        /// What the filesystem reported.
        source: std::io::Error,
    },
}

impl ArchiveError {
    /// Returns what somebody holding this archive should try next.
    ///
    /// Carried with the error rather than composed at each boundary, because the
    /// command line and the workbench are both reporting the same problem to the
    /// same person and inventing two answers to it helps nobody.
    pub fn advice(&self) -> &'static [&'static str] {
        match self {
            Self::Unreadable { .. } => &[
                "Check the file downloaded completely, then try again.",
                "A design is shared as the .zip written by `optimist export`.",
            ],
            Self::NotADesign => &[
                "The archive must hold a _system.yaml, either at its root or inside one folder.",
                "Ask whoever sent it to run `optimist export` on the design directory.",
            ],
            Self::Misplaced { .. } => &[
                "A design holds _system.yaml, components/, component-types/, and mutators/, and nothing else.",
                "Repack the design with `optimist export` rather than zipping the folder by hand.",
            ],
            Self::TooManyEntries { .. } | Self::TooLarge { .. } => &[
                "This is far larger than a design should be; treat the file as untrustworthy.",
                "Ask for the design to be shared again, packed with `optimist export`.",
            ],
            Self::Invalid { .. } => &[
                "The message names the document at fault; a newer build may be needed to read it.",
                "Run `optimist check` against the sender's design to see the same problem there.",
            ],
            Self::Io { .. } => &["Check that the destination exists and can be written to."],
        }
    }
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { .. } => formatter.write_str("this file is not a readable archive"),
            Self::NotADesign => {
                formatter.write_str("this archive contains no _system.yaml, so it is not a design")
            }
            Self::Misplaced { entry } => write!(
                formatter,
                "'{entry}' is not part of a design; a design holds _system.yaml, components/, component-types/, and mutators/"
            ),
            Self::TooManyEntries { limit } => write!(
                formatter,
                "this archive declares more than {limit} files, which no design does"
            ),
            Self::TooLarge { limit } => write!(
                formatter,
                "this archive unpacks to more than {limit} bytes, which no design does"
            ),
            Self::Invalid { source } => {
                write!(formatter, "this archive does not hold a design: {source}")
            }
            Self::Io { path, source } => write!(formatter, "{path}: {source}"),
        }
    }
}

impl std::error::Error for ArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid { source } => Some(source),
            _ => None,
        }
    }
}
