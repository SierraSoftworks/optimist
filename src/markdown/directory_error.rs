use std::path::PathBuf;

use thiserror::Error;

use super::{ImportError, MarkdownError};

/// Failures while reading or publishing a Markdown project directory.
#[derive(Debug, Error)]
pub enum DirectoryError {
    /// A filesystem operation failed at a source-aware path.
    #[error("could not {operation} {path}: {message}")]
    Io {
        /// Operation attempted by the directory transport.
        operation: &'static str,
        /// Filesystem path at which the operation failed.
        path: PathBuf,
        /// Operating-system diagnostic.
        message: String,
    },
    /// A bounded Markdown file failed local parsing or rendering.
    #[error(transparent)]
    Markdown(#[from] MarkdownError),
    /// The complete parsed collection failed project-level validation.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// A document directory exceeds the collection-level file bound.
    #[error("{path}: Markdown snapshot exceeds the {maximum} file limit")]
    TooManyFiles {
        /// Directory whose entries exceeded the bound.
        path: PathBuf,
        /// Maximum accepted entity and scenario document count.
        maximum: usize,
    },
}

pub(super) fn io(
    operation: &'static str,
    path: impl Into<PathBuf>,
    error: std::io::Error,
) -> DirectoryError {
    DirectoryError::Io {
        operation,
        path: path.into(),
        message: error.to_string(),
    }
}
