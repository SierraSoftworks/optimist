//! A directory of designs, any of which a client may open.
//!
//! # Why a workspace rather than a process per design
//!
//! An engineer rarely reasons about one system in isolation. The design being
//! changed sits next to the one it depends on and the one that replaced it last
//! year, and being able to open each without restarting anything is the
//! difference between a tool and a batch job.
//!
//! Designs are otherwise independent. Each has its own in-memory state and its
//! own change feed, so editing one neither blocks nor notifies anyone working on
//! another. Nothing is shared but the directory they happen to live under.
//!
//! # Opening on demand
//!
//! Listing a workspace reads only the header of each design, which is cheap and
//! stays cheap as designs grow. A design is fully loaded the first time someone
//! opens it and then stays loaded, because everyone editing it has to share one
//! copy for the change feed to mean anything.
//!
//! A design that cannot be read appears in the listing with the reason it could
//! not. Hiding it would be worse: an engineer who cannot find a design they know
//! exists has no way to discover that its file is malformed, whereas one who can
//! see the error can fix it.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::Serialize;

use crate::system::{SchemaError, SystemDocument, safe_identifier};

use super::Session;

/// Identifies one design within a workspace.
///
/// The identifier is the design's directory name, so what a client asks for and
/// what an engineer sees on disk are the same string.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DesignId(String);

impl DesignId {
    /// Creates an identifier, rejecting anything that could name another directory.
    pub fn new(id: impl Into<String>) -> Result<Self, WorkspaceError> {
        let id = id.into();
        safe_identifier(&id)
            .map(|_| Self(id.clone()))
            .map_err(|_| WorkspaceError::UnsafeIdentifier { value: id })
    }

    /// Borrows the identifier's text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DesignId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// What a listing says about one design.
#[derive(Clone, Debug, Serialize)]
pub struct DesignSummary {
    /// Directory name, and the identifier a client opens it by.
    pub id: DesignId,
    /// Human-readable name, where the design could be read.
    pub name: String,
    /// What the design is for.
    pub summary: String,
    /// Why the design could not be read, where it could not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unreadable: Option<String>,
}

impl DesignSummary {
    /// Reports whether this design can be opened.
    pub fn is_readable(&self) -> bool {
        self.unreadable.is_none()
    }
}

/// Why a design could not be opened.
#[derive(Debug)]
pub enum WorkspaceError {
    /// The workspace root is missing or unreadable.
    Root {
        /// The path being read.
        path: String,
        /// What the filesystem reported.
        source: std::io::Error,
    },
    /// No design goes by that name.
    NotFound {
        /// The identifier requested.
        id: String,
    },
    /// An identifier could name something outside the workspace.
    UnsafeIdentifier {
        /// The rejected identifier.
        value: String,
    },
    /// A design already goes by that name.
    AlreadyExists {
        /// The identifier requested.
        id: String,
    },
    /// A design could not be written.
    Malformed {
        /// The identifier requested.
        id: String,
        /// What went wrong while rendering it.
        message: String,
    },
    /// The design exists but could not be read.
    Unreadable {
        /// The identifier requested.
        id: String,
        /// Why it could not be read.
        source: SchemaError,
    },
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root { path, source } => write!(formatter, "{path}: {source}"),
            Self::NotFound { id } => write!(formatter, "no design named '{id}'"),
            Self::UnsafeIdentifier { value } => write!(
                formatter,
                "'{value}' cannot name a design; use lower-case letters, digits, hyphens, and underscores"
            ),
            Self::Unreadable { id, source } => {
                write!(formatter, "design '{id}' could not be read: {source}")
            }
            Self::AlreadyExists { id } => {
                write!(formatter, "a design named '{id}' already exists")
            }
            Self::Malformed { id, message } => {
                write!(formatter, "design '{id}' could not be written: {message}")
            }
        }
    }
}

impl std::error::Error for WorkspaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Root { source, .. } => Some(source),
            Self::Unreadable { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The designs a server is serving.
pub struct Workspace {
    root: PathBuf,
    open: RwLock<BTreeMap<DesignId, Arc<Session>>>,
}

impl Workspace {
    /// Serves the designs under `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            open: RwLock::new(BTreeMap::new()),
        }
    }

    /// Borrows the directory this workspace serves.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lists every design, readable or not, by directory name.
    pub fn designs(&self) -> Result<Vec<DesignSummary>, WorkspaceError> {
        let entries = fs::read_dir(&self.root).map_err(|source| WorkspaceError::Root {
            path: self.root.display().to_string(),
            source,
        })?;
        let mut designs = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| WorkspaceError::Root {
                path: self.root.display().to_string(),
                source,
            })?;
            if !entry.path().join("_system.yaml").is_file() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(id) = DesignId::new(name) else {
                continue;
            };
            designs.push(self.summarise(id));
        }
        designs.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(designs)
    }

    /// Creates an empty design and returns its session.
    ///
    /// The directory is written before the session is opened, so a design exists
    /// on disk from the moment it is named rather than only once somebody edits
    /// it. A design that is created and abandoned is then a visible empty
    /// directory rather than nothing at all, which is the behaviour that makes
    /// the workspace listing trustworthy.
    ///
    /// Refuses to overwrite. Reusing an existing identifier is far more likely to
    /// be a mistake than an intention, and the cost of being wrong is somebody
    /// else's design.
    pub fn create(
        &self,
        id: &DesignId,
        name: &str,
        summary: &str,
    ) -> Result<Arc<Session>, WorkspaceError> {
        let directory = self.root.join(id.as_str());
        if directory.join("_system.yaml").exists() {
            return Err(WorkspaceError::AlreadyExists { id: id.to_string() });
        }
        fs::create_dir_all(directory.join("components")).map_err(|source| {
            WorkspaceError::Root {
                path: directory.display().to_string(),
                source,
            }
        })?;
        let document = SystemDocument {
            schema_version: crate::system::SCHEMA_VERSION,
            name: name.to_owned(),
            summary: summary.to_owned(),
            scratchpad: Vec::new(),
            scale_units: Vec::new(),
            interventions: Vec::new(),
        };
        let rendered =
            serde_yaml_ng::to_string(&document).map_err(|error| WorkspaceError::Malformed {
                id: id.to_string(),
                message: error.to_string(),
            })?;
        fs::write(directory.join("_system.yaml"), rendered).map_err(|source| {
            WorkspaceError::Root {
                path: directory.display().to_string(),
                source,
            }
        })?;
        self.session(id)
    }

    /// Deletes a design and everything under its directory.
    ///
    /// The session is dropped and abandoned before the files go, so a write that
    /// was already waiting for the design to settle cannot put the directory
    /// back. Anyone still holding a session keeps reading the design they had;
    /// they are looking at something that no longer exists, which is the same
    /// position they would be in had it been deleted a moment later.
    pub fn remove(&self, id: &DesignId) -> Result<(), WorkspaceError> {
        let directory = self.root.join(id.as_str());
        if !directory.join("_system.yaml").is_file() {
            return Err(WorkspaceError::NotFound { id: id.to_string() });
        }
        if let Some(session) = self.open().remove(id) {
            session.discard();
        }
        fs::remove_dir_all(&directory).map_err(|source| WorkspaceError::Root {
            path: directory.display().to_string(),
            source,
        })
    }

    /// Opens a design, loading it if this is the first request for it.
    ///
    /// Everyone editing a design shares the returned session, which is what lets
    /// one editor's change reach another's screen.
    pub fn session(&self, id: &DesignId) -> Result<Arc<Session>, WorkspaceError> {
        if let Some(session) = self.opened(id) {
            return Ok(session);
        }
        let directory = self.root.join(id.as_str());
        if !directory.join("_system.yaml").is_file() {
            return Err(WorkspaceError::NotFound { id: id.to_string() });
        }
        let session =
            Arc::new(
                Session::open(&directory).map_err(|source| WorkspaceError::Unreadable {
                    id: id.to_string(),
                    source,
                })?,
            );
        // Another request may have opened the same design while this one was
        // reading it; the first to arrive wins so that everyone shares one copy.
        Ok(Arc::clone(
            self.open()
                .entry(id.clone())
                .or_insert_with(|| Arc::clone(&session)),
        ))
    }

    /// Returns the designs already loaded, in identifier order.
    pub fn loaded(&self) -> Vec<(DesignId, Arc<Session>)> {
        self.open()
            .iter()
            .map(|(id, session)| (id.clone(), Arc::clone(session)))
            .collect()
    }

    /// Writes every loaded design that has settled, returning how many were written.
    pub fn persist_due(&self) -> Result<usize, SchemaError> {
        let mut written = 0;
        for (_, session) in self.loaded() {
            if session.persist_if_due()? {
                written += 1;
            }
        }
        Ok(written)
    }

    /// Writes every loaded design with unsaved edits, whatever their state.
    ///
    /// Called when a server is shutting down, where the alternative is losing
    /// edits that were merely waiting for a pause that will never come.
    pub fn persist_all(&self) -> Result<usize, SchemaError> {
        let mut written = 0;
        for (_, session) in self.loaded() {
            if session.pending() {
                session.persist()?;
                written += 1;
            }
        }
        Ok(written)
    }

    fn summarise(&self, id: DesignId) -> DesignSummary {
        let path = self.root.join(id.as_str()).join("_system.yaml");
        match read_header(&path) {
            Ok(document) => DesignSummary {
                id,
                name: document.name,
                summary: document.summary,
                unreadable: None,
            },
            Err(error) => DesignSummary {
                name: id.to_string(),
                summary: String::new(),
                unreadable: Some(error),
                id,
            },
        }
    }

    fn opened(&self, id: &DesignId) -> Option<Arc<Session>> {
        self.open().get(id).map(Arc::clone)
    }

    fn open(&self) -> std::sync::RwLockWriteGuard<'_, BTreeMap<DesignId, Arc<Session>>> {
        self.open
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Reads only what a listing needs, so listing stays cheap as designs grow.
fn read_header(path: &Path) -> Result<SystemDocument, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_yaml_ng::from_str(&source).map_err(|error| error.to_string())
}
