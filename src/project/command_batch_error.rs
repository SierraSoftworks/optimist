use thiserror::Error;
use uuid::Uuid;

/// Failures which reject an atomic command batch before durable publication.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum CommandBatchError {
    /// Atomic batches must contain at least one command.
    #[error("a command batch cannot be empty")]
    Empty,
    /// The submitted command count exceeds the bounded transaction limit.
    #[error("command batch contains {count} commands; maximum is {maximum}")]
    TooLarge {
        /// Commands submitted by the caller.
        count: usize,
        /// Maximum accepted in one atomic transaction.
        maximum: usize,
    },
    /// The project revision cannot represent every command in the batch.
    #[error("command batch would exhaust the project revision space")]
    RevisionSpaceExhausted,
    /// A retry reused a batch UUID with different commands or compensation lineage.
    #[error("command batch {0} was already used for different content")]
    RequestConflict(Uuid),
    /// No committed forward batch exists for the requested compensation target.
    #[error("command batch {0} does not exist in retained history")]
    NotFound(Uuid),
    /// The selected forward batch was already compensated by another batch.
    #[error("command batch {batch} was already compensated by {compensation}")]
    AlreadyCompensated {
        /// Original batch selected for undo.
        batch: Uuid,
        /// Existing compensation batch.
        compensation: Uuid,
    },
    /// Compensation batches cannot themselves be selected for automatic lineage linking.
    #[error("command batch {0} is already a compensation batch")]
    CompensationTarget(Uuid),
}
