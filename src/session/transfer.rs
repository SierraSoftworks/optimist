//! Sharing a design out of a workspace, and taking one in.
//!
//! An engineer who has modelled something worth showing a colleague has no way
//! to hand it over that does not involve access to the same directory. Packing a
//! design into one file makes it something that can be attached to a review,
//! committed alongside a proposal, or sent to somebody who has never run this
//! tool at all.
//!
//! Taking one back in is the same operation reversed, with the difference that
//! matters: the file came from somewhere else. It is validated in full, in a
//! scratch directory, before anything in the workspace changes.

use std::sync::Arc;

use crate::system::{StagedDesign, pack_system};

use super::{DesignId, Session, Workspace, WorkspaceError};

impl Workspace {
    /// Packs a design into an archive that can be shared.
    ///
    /// Unsaved edits are written first, so what a colleague receives is the
    /// design as it is on screen rather than as it was when the last quiet
    /// period ended.
    pub fn export(&self, id: &DesignId) -> Result<Vec<u8>, WorkspaceError> {
        let session = self.session(id)?;
        if session.pending() {
            session
                .persist()
                .map_err(|source| WorkspaceError::Unreadable {
                    id: id.to_string(),
                    source,
                })?;
        }
        pack_system(&self.root().join(id.as_str()))
            .map_err(|source| WorkspaceError::Archive { source })
    }

    /// Stores an archive as the design `id`, and opens it.
    ///
    /// Refuses to overwrite unless asked to. An import that silently replaced a
    /// colleague's work because two designs were exported from directories with
    /// the same name would be the most expensive mistake this tool could make,
    /// so replacing is something a person says rather than something a filename
    /// decides.
    pub fn import(
        &self,
        id: &DesignId,
        archive: &[u8],
        replace: bool,
    ) -> Result<Arc<Session>, WorkspaceError> {
        let directory = self.root().join(id.as_str());
        if !replace && directory.join("_system.yaml").is_file() {
            return Err(WorkspaceError::AlreadyExists { id: id.to_string() });
        }

        // Staged before anything is removed, so an archive that turns out not to
        // hold a design leaves the one being replaced exactly where it was.
        let staged = StagedDesign::stage(archive, &directory)
            .map_err(|source| WorkspaceError::Archive { source })?;
        if directory.exists() {
            self.remove(id)?;
        }
        staged
            .install(&directory)
            .map_err(|source| WorkspaceError::Archive { source })?;
        self.session(id)
    }
}
