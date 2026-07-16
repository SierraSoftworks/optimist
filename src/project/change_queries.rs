use crate::{
    command::{ChangeSet, ChangeSetReplay},
    domain::ProjectId,
};

use super::{ProjectCatalog, ProjectError};

impl ProjectCatalog {
    /// Retrieves the committed change at one exact project revision.
    pub fn get_change(
        &self,
        project: &ProjectId,
        revision: u64,
    ) -> Result<Option<ChangeSet>, ProjectError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        Ok(entry.changes.get(&revision).cloned())
    }

    /// Replays committed changes after an exclusive project revision.
    pub fn replay_changes(
        &self,
        project: &ProjectId,
        after_revision: u64,
    ) -> Result<ChangeSetReplay, ProjectError> {
        let entry = self
            .projects
            .get(project)
            .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
        if after_revision > entry.project.revision {
            return Err(ProjectError::InvalidReplayRevision {
                requested: after_revision,
                current: entry.project.revision,
            });
        }
        if after_revision < entry.change_history_start {
            return Err(ProjectError::ChangeHistoryGap {
                requested: after_revision,
                available_after: entry.change_history_start,
            });
        }
        Ok(ChangeSetReplay {
            after_revision,
            current_revision: entry.project.revision,
            changes: entry
                .changes
                .range((
                    std::ops::Bound::Excluded(after_revision),
                    std::ops::Bound::Unbounded,
                ))
                .map(|(_, change)| change.clone())
                .collect(),
        })
    }
}
