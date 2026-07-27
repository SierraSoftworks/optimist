//! Turning failures into responses a client can act on.

use std::net::SocketAddr;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::session::{MutationError, WorkspaceError};
use crate::system::EvaluationError;

/// Why a request could not be served.
#[derive(Debug)]
pub enum ApiError {
    /// The listening address could not be taken.
    Bind {
        /// The address that was requested.
        address: SocketAddr,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The server stopped unexpectedly.
    Serve {
        /// What the server reported.
        source: std::io::Error,
    },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bind { address, source } => {
                write!(formatter, "could not listen on {address}: {source}")
            }
            Self::Serve { source } => write!(formatter, "the server stopped: {source}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Bind { source, .. } | Self::Serve { source } => Some(source),
        }
    }
}

/// What a client is told when a request cannot be served.
///
/// The advice matters as much as the message. A client that is told only that
/// something failed can do nothing but show a dialog, whereas one told what
/// would make the request succeed can often fix it without a person.
#[derive(Debug, Serialize)]
pub(super) struct Failure {
    message: String,
    advice: &'static str,
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

/// A failure carrying the status that describes it.
pub(super) struct Rejected(StatusCode, Failure);

impl IntoResponse for Rejected {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

impl From<WorkspaceError> for Rejected {
    fn from(error: WorkspaceError) -> Self {
        let status = match error {
            WorkspaceError::NotFound { .. } => StatusCode::NOT_FOUND,
            WorkspaceError::UnsafeIdentifier { .. } => StatusCode::BAD_REQUEST,
            WorkspaceError::Root { .. } | WorkspaceError::Unreadable { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let advice = match error {
            WorkspaceError::NotFound { .. } => "List the workspace to see which designs exist.",
            WorkspaceError::UnsafeIdentifier { .. } => {
                "Use the directory name of a design, in lower case."
            }
            WorkspaceError::Root { .. } => {
                "Check that the server can read the directory it was given."
            }
            WorkspaceError::Unreadable { .. } => {
                "Fix the file named in the message, then reload the design."
            }
        };
        Self(
            status,
            Failure {
                message: error.to_string(),
                advice,
            },
        )
    }
}

impl From<MutationError> for Rejected {
    fn from(error: MutationError) -> Self {
        Self(
            StatusCode::CONFLICT,
            Failure {
                message: error.to_string(),
                advice: "Reload the design; another editor may have removed what this change refers to.",
            },
        )
    }
}

impl From<EvaluationError> for Rejected {
    fn from(error: EvaluationError) -> Self {
        Self(
            StatusCode::UNPROCESSABLE_ENTITY,
            Failure {
                message: error.to_string(),
                advice: "The design is incomplete or inconsistent; the message names what to fix.",
            },
        )
    }
}
