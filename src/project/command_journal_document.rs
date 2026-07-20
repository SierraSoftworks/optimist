use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    command::{CommandBatchRequest, CommandRequest},
    domain::ProjectId,
};

use super::CatalogPersistenceError;

const JOURNAL_SCHEMA_VERSION: u32 = 2;

#[derive(Deserialize, Serialize)]
struct CommandJournal {
    schema_version: u32,
    mutation: PendingMutation,
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

#[derive(Deserialize)]
struct LegacyPendingCommand {
    project: ProjectId,
    request: CommandRequest,
}

pub(super) fn encode(mutation: PendingMutation) -> Vec<u8> {
    serde_json::to_vec(&CommandJournal {
        schema_version: JOURNAL_SCHEMA_VERSION,
        mutation,
    })
    .expect("pending commands serialize")
}

pub(super) fn decode(
    bytes: &[u8],
    path: &Path,
) -> Result<PendingMutation, CatalogPersistenceError> {
    let document: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|source| CatalogPersistenceError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    let version = document
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|version| u32::try_from(version).ok())
        .unwrap_or_default();
    match version {
        1 => {
            let legacy: LegacyPendingCommand =
                serde_json::from_value(document).map_err(|source| {
                    CatalogPersistenceError::Json {
                        path: path.to_path_buf(),
                        source,
                    }
                })?;
            Ok(PendingMutation::Command {
                project: legacy.project,
                request: Box::new(legacy.request),
            })
        }
        JOURNAL_SCHEMA_VERSION => serde_json::from_value::<CommandJournal>(document)
            .map(|journal| journal.mutation)
            .map_err(|source| CatalogPersistenceError::Json {
                path: path.to_path_buf(),
                source,
            }),
        version => Err(CatalogPersistenceError::UnsupportedJournalSchema(version)),
    }
}
