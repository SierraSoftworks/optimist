//! What a command tells the workbench when it will not do what was asked.

use axum::{body::to_bytes, http::StatusCode, response::Response};
use serde::Serialize;
use serde_json::Value;

/// How much of a refusal is worth reading before giving up on it.
const LIMIT: usize = 64 * 1024;

/// A refusal, in the shape the server would have refused in.
///
/// The workbench reads the same fields whichever transport carried them, so a
/// person editing in a window is told what a person editing in a browser would
/// have been told, with the same advice about what to do next.
#[derive(Debug, Serialize)]
pub(crate) struct Failure {
    /// The status the same request would have been answered with.
    status: u16,
    message: String,
    advice: Vec<String>,
}

impl Failure {
    /// A fault in this process rather than in what was asked of it.
    pub(super) fn fault(message: impl Into<String>, advice: &str) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message, advice)
    }

    /// A request this process would not put to the API at all.
    pub(super) fn refused(status: StatusCode, message: impl Into<String>, advice: &str) -> Self {
        Self::new(status, message, advice)
    }

    fn new(status: StatusCode, message: impl Into<String>, advice: &str) -> Self {
        Self {
            status: status.as_u16(),
            message: message.into(),
            advice: vec![advice.to_owned()],
        }
    }

    /// Reads a refusal the API has already written out.
    ///
    /// Going through the rendered response rather than the error behind it is
    /// what keeps one table of statuses and advice for both transports.
    pub(super) async fn read(response: Response) -> Self {
        let status = response.status();
        let body = to_bytes(response.into_body(), LIMIT).await.unwrap_or_default();
        let failure: Option<Value> = serde_json::from_slice(&body).ok();
        let message = failure
            .as_ref()
            .and_then(|failure| failure.get("message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("The request failed with status {status}."));
        Self {
            status: status.as_u16(),
            message,
            advice: advice(failure.as_ref().and_then(|failure| failure.get("advice"))),
        }
    }
}

/// One suggestion is sent as a string and several as a list.
fn advice(advice: Option<&Value>) -> Vec<String> {
    match advice {
        Some(Value::String(line)) => vec![line.clone()],
        Some(Value::Array(lines)) => lines
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}
