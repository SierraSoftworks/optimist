use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    command::{CommandBatchRequest, CommandRequest},
    domain::ProjectId,
};

use super::CatalogPersistenceError;

const JOURNAL_SCHEMA_VERSION: u32 = 3;

#[derive(Deserialize, Serialize)]
struct CommandJournal {
    schema_version: u32,
    mutations: Vec<PendingMutation>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum PendingMutation {
    Command {
        project: ProjectId,
        request: Box<CommandRequest>,
    },
    Batch {
        project: ProjectId,
        request: CommandBatchRequest,
        compensates: Option<uuid::Uuid>,
    },
}

pub(super) fn encode(mutations: Vec<PendingMutation>) -> Vec<u8> {
    serde_json::to_vec(&CommandJournal {
        schema_version: JOURNAL_SCHEMA_VERSION,
        mutations,
    })
    .expect("pending commands serialize")
}

pub(super) fn decode(
    bytes: &[u8],
    path: &Path,
) -> Result<Vec<PendingMutation>, CatalogPersistenceError> {
    let document: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|source| CatalogPersistenceError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    let journal: CommandJournal =
        serde_json::from_value(document).map_err(|source| CatalogPersistenceError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    if journal.schema_version != JOURNAL_SCHEMA_VERSION {
        return Err(CatalogPersistenceError::UnsupportedJournalSchema(
            journal.schema_version,
        ));
    }
    Ok(journal.mutations)
}
