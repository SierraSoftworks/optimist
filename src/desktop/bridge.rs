//! Requests from the webview, answered by the API itself.
//!
//! The desktop application has no socket, so a request is put through the same
//! router `optimist serve` puts behind one. Every handler, refusal and status is
//! therefore the server's, which is what keeps the two ways of reaching a design
//! from answering differently, and what keeps a change to one from having to be
//! made twice.

use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use serde_json::Value;
use tauri::State;
use tower::ServiceExt;

use super::{Desktop, Failure};

/// The only prefix a command may reach.
///
/// The webview runs the workbench and nothing else. This is a bound on what a
/// page that had been tampered with could ask for, rather than on what the
/// workbench itself asks for.
const PREFIX: &str = "/api/v1/";

/// How much of an answer will be carried back across the boundary.
///
/// A solved design asked for every step of a long horizon is the largest thing
/// the API sends, and is well inside this.
const LIMIT: usize = 64 << 20;

#[tauri::command]
pub(super) async fn api_call(
    desktop: State<'_, Desktop>,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, Failure> {
    call(desktop.inner(), &method, &path, body).await
}

pub(super) async fn call(
    desktop: &Desktop,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, Failure> {
    let response = route(desktop, request(method, path, body)?).await?;
    let body = read(response).await?;
    if body.is_empty() {
        // A deletion succeeds with nothing to say, and there is no JSON in an
        // empty body to fail to parse.
        return Ok(Value::Null);
    }
    serde_json::from_slice(&body).map_err(|error| {
        Failure::fault(
            format!("The API answered with something that is not JSON: {error}"),
            "This is a defect in the application rather than in the design.",
        )
    })
}

/// Puts one request to the API and hands back what it answered.
///
/// A refusal is read here rather than returned, so that every caller reports one
/// the same way and only success reaches the code that knows what to do with it.
pub(super) async fn route(desktop: &Desktop, request: Request<Body>) -> Result<Response, Failure> {
    let response = desktop
        .service
        .router()
        .into_service::<Body>()
        .oneshot(request)
        .await
        .map_err(|error| {
            Failure::fault(
                format!("The request could not be routed: {error}"),
                "This is a defect in the application rather than in the design.",
            )
        })?;
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(Failure::read(response).await)
    }
}

/// Reads an answer the API has finished writing.
pub(super) async fn read(response: Response) -> Result<Vec<u8>, Failure> {
    to_bytes(response.into_body(), LIMIT)
        .await
        .map(|body| body.to_vec())
        .map_err(|error| {
            Failure::fault(
                format!("The answer could not be read: {error}"),
                "Ask for fewer steps, or fewer samples, and try again.",
            )
        })
}

/// Builds a request from what the webview asked for.
fn request(method: &str, path: &str, body: Option<Value>) -> Result<Request<Body>, Failure> {
    if !path.starts_with(PREFIX) {
        return Err(Failure::refused(
            StatusCode::FORBIDDEN,
            format!("'{path}' is not part of the API."),
            "This is a defect in the application rather than in the design.",
        ));
    }
    let method = Method::from_bytes(method.as_bytes()).map_err(|_| {
        Failure::refused(
            StatusCode::METHOD_NOT_ALLOWED,
            format!("'{method}' is not a method."),
            "This is a defect in the application rather than in the design.",
        )
    })?;

    let builder = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ACCEPT, "application/json");
    let built = match body {
        Some(value) => {
            let encoded = serde_json::to_vec(&value).map_err(|error| {
                Failure::fault(
                    format!("The request could not be written: {error}"),
                    "This is a defect in the application rather than in the design.",
                )
            })?;
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(encoded))
        }
        None => builder.body(Body::empty()),
    };
    built.map_err(|error| {
        Failure::refused(
            StatusCode::BAD_REQUEST,
            format!("'{path}' is not a path the API could be asked about: {error}"),
            "This is a defect in the application rather than in the design.",
        )
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use rstest::rstest;

    use crate::desktop::tests::workspace;

    use super::*;

    /// Starts a design the way the workbench would.
    pub(crate) async fn create(desktop: &Desktop, id: &str) {
        call(
            desktop,
            "POST",
            "/api/v1/designs",
            Some(serde_json::json!({ "id": id, "name": id, "summary": "" })),
        )
        .await
        .expect("starts a design");
    }

    #[rstest]
    #[case("/health")]
    #[case("/etc/passwd")]
    #[case("../api/v1/designs")]
    fn refuses_paths_outside_the_api(#[case] path: &str) {
        let failure = request("GET", path, None).expect_err("is refused");
        assert_eq!(
            serde_json::to_value(&failure).expect("serialises")["status"],
            403
        );
    }

    #[test]
    fn refuses_something_that_is_not_a_method() {
        assert!(request("GET DESIGNS", "/api/v1/designs", None).is_err());
    }

    #[tokio::test]
    async fn answers_the_way_the_server_would() {
        let (desktop, _root) = workspace();
        let health = call(&desktop, "GET", "/api/v1/health", None)
            .await
            .expect("answers");

        assert_eq!(health["status"], "ok");
    }

    /// An unknown endpoint is a refusal with advice, not a page or a panic.
    #[tokio::test]
    async fn carries_a_refusal_back_with_its_advice() {
        let (desktop, _root) = workspace();
        let failure = call(&desktop, "GET", "/api/v1/nowhere", None)
            .await
            .expect_err("is refused");

        let reported = serde_json::to_value(&failure).expect("serialises");
        assert_eq!(reported["status"], 404);
        assert_eq!(reported["message"], "No such endpoint.");
        assert!(!reported["advice"].as_array().expect("advice").is_empty());
    }

    /// A design edited through the window is a design the server would agree on.
    #[tokio::test]
    async fn writes_as_well_as_reads() {
        let (desktop, _root) = workspace();
        create(&desktop, "checkout").await;

        call(
            &desktop,
            "POST",
            "/api/v1/designs/checkout/mutations",
            Some(serde_json::json!({ "mutations": [{
                "kind": "set_scratchpad_entry",
                "entry": { "name": "peak_rate", "expression": "50", "unit": "op/s", "summary": "" },
            }] })),
        )
        .await
        .expect("applies");

        let design = call(&desktop, "GET", "/api/v1/designs/checkout", None)
            .await
            .expect("reads it back");
        assert_eq!(design["sequence"], 1);
    }

    /// A deletion answers with no content, which is not a failure to parse.
    #[tokio::test]
    async fn reads_an_answer_with_no_body() {
        let (desktop, _root) = workspace();
        create(&desktop, "checkout").await;

        let removed = call(&desktop, "DELETE", "/api/v1/designs/checkout", None)
            .await
            .expect("deletes it");
        assert!(removed.is_null());
    }
}
