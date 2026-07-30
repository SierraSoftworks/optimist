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
    advice: Advice,
}

/// Guidance offered with a refusal.
///
/// Most refusals have one thing to suggest and say it; an archive that was
/// rejected usually has two, because what to do about it depends on whether the
/// sender or the recipient is the one who can fix it.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Advice {
    One(&'static str),
    Several(&'static [&'static str]),
}

impl From<&'static str> for Advice {
    fn from(line: &'static str) -> Self {
        Self::One(line)
    }
}

impl From<&'static [&'static str]> for Advice {
    fn from(lines: &'static [&'static str]) -> Self {
        Self::Several(lines)
    }
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
            WorkspaceError::AlreadyExists { .. } => StatusCode::CONFLICT,
            WorkspaceError::Archive { ref source } => archive_status(source),
            WorkspaceError::Root { .. }
            | WorkspaceError::Unreadable { .. }
            | WorkspaceError::Malformed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let advice: Advice = match error {
            WorkspaceError::NotFound { .. } => {
                "List the workspace to see which designs exist.".into()
            }
            WorkspaceError::UnsafeIdentifier { .. } => {
                "Use the directory name of a design, in lower case.".into()
            }
            WorkspaceError::AlreadyExists { .. } => {
                "Open the existing design, or choose another identifier.".into()
            }
            WorkspaceError::Root { .. } => {
                "Check that the server can read the directory it was given.".into()
            }
            WorkspaceError::Unreadable { .. } => {
                "Fix the file named in the message, then reload the design.".into()
            }
            WorkspaceError::Malformed { .. } => {
                "This is a defect in the server rather than in the design.".into()
            }
            WorkspaceError::Archive { ref source } => source.advice().into(),
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

/// Separates an archive this server could not accept from one it could not store.
///
/// Everything about an untrusted file is the sender's problem to fix and is
/// reported as such; only a filesystem that would not cooperate belongs to the
/// server, and telling a client to repack the file in that case would send them
/// after a fault they cannot reach.
fn archive_status(error: &crate::system::ArchiveError) -> StatusCode {
    match error {
        crate::system::ArchiveError::Io { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        crate::system::ArchiveError::TooLarge { .. }
        | crate::system::ArchiveError::TooManyEntries { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

impl From<MutationError> for Rejected {
    fn from(error: MutationError) -> Self {
        Self(
            StatusCode::CONFLICT,
            Failure {
                message: error.to_string(),
                advice:
                    "Reload the design; another editor may have removed what this change refers to."
                        .into(),
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
                advice: "The design is incomplete or inconsistent; the message names what to fix."
                    .into(),
            },
        )
    }
}
