use std::collections::BTreeMap;

use crate::{
    command::{
        ChangeSet, CommandBatchRequest, CommandBatchResult, CommandResult, child_request_id,
    },
    domain::ProjectId,
};

use super::{CommandBatchError, ProjectCatalog, ProjectError};

struct BatchHistory {
    compensates: Option<uuid::Uuid>,
    commands: usize,
    last_revision: u64,
}

pub(super) fn validate_persisted_batches(changes: &[ChangeSet]) -> Result<(), &'static str> {
    let mut batches = BTreeMap::<uuid::Uuid, BatchHistory>::new();
    let mut compensations = BTreeMap::<uuid::Uuid, uuid::Uuid>::new();
    for change in changes {
        let Some(batch) = change.batch_id else {
            if change.compensates.is_some() {
                return Err("a compensation ChangeSet has no batch ID");
            }
            continue;
        };
        let history = batches.entry(batch).or_insert(BatchHistory {
            compensates: change.compensates,
            commands: 0,
            last_revision: change.base_revision,
        });
        if history.compensates != change.compensates
            || history.last_revision.checked_add(1) != Some(change.project_revision)
            || child_request_id(batch, history.commands) != change.request_id
        {
            return Err("batch ChangeSets are not contiguous and deterministic");
        }
        history.commands += 1;
        history.last_revision = change.project_revision;
        if let Some(target) = change.compensates
            && compensations
                .insert(target, batch)
                .is_some_and(|owner| owner != batch)
        {
            return Err("a command batch has multiple compensation batches");
        }
    }
    for (batch, history) in &batches {
        if let Some(target) = history.compensates {
            if target == *batch {
                return Err("a command batch compensates itself");
            }
            let Some(original) = batches.get(&target) else {
                return Err("a compensation target is absent from retained history");
            };
            if original.compensates.is_some() {
                return Err("a compensation batch targets another compensation");
            }
        }
    }
    Ok(())
}

pub(super) fn existing_batch(
    catalog: &ProjectCatalog,
    project: &ProjectId,
    request: &CommandBatchRequest,
    compensates: Option<uuid::Uuid>,
) -> Result<Option<CommandBatchResult>, ProjectError> {
    let entry = catalog
        .projects
        .get(project)
        .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
    let changes = entry
        .changes
        .values()
        .filter(|change| change.batch_id == Some(request.request_id))
        .collect::<Vec<_>>();
    if changes.is_empty() {
        return Ok(None);
    }
    let same_content = changes.len() == request.commands.len()
        && changes
            .iter()
            .map(|change| &change.command)
            .eq(&request.commands)
        && changes
            .iter()
            .all(|change| change.compensates == compensates);
    if !same_content {
        return Err(CommandBatchError::RequestConflict(request.request_id).into());
    }
    let results = changes
        .iter()
        .map(|change| CommandResult {
            request_id: change.request_id,
            project_revision: change.project_revision,
            outcome: change.outcome.clone(),
        })
        .collect::<Vec<_>>();
    Ok(Some(CommandBatchResult {
        request_id: request.request_id,
        base_revision: changes[0].base_revision,
        project_revision: changes.last().unwrap().project_revision,
        compensates,
        results,
    }))
}

pub(super) fn validate_compensation(
    catalog: &ProjectCatalog,
    project: &ProjectId,
    target: Option<uuid::Uuid>,
) -> Result<(), ProjectError> {
    let Some(target) = target else {
        return Ok(());
    };
    let entry = catalog
        .projects
        .get(project)
        .ok_or_else(|| ProjectError::NotFound(project.clone()))?;
    let original = entry
        .changes
        .values()
        .find(|change| change.batch_id == Some(target))
        .ok_or(CommandBatchError::NotFound(target))?;
    if original.compensates.is_some() {
        return Err(CommandBatchError::CompensationTarget(target).into());
    }
    if let Some(change) = entry
        .changes
        .values()
        .find(|change| change.compensates == Some(target))
    {
        return Err(CommandBatchError::AlreadyCompensated {
            batch: target,
            compensation: change
                .batch_id
                .expect("compensation changes belong to a batch"),
        }
        .into());
    }
    Ok(())
}
