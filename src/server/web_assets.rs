use std::path::Path;

use axum::{
    Json, Router,
    http::{HeaderValue, StatusCode, header::CACHE_CONTROL},
    middleware,
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::json;
use tower::ServiceBuilder;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
const REVALIDATE_CACHE: &str = "no-cache";
const NO_STORE_CACHE: &str = "no-store";

pub(super) fn with_workbench(api: Router, root: Option<&Path>) -> Router {
    let Some(root) = root.filter(|root| root.join("index.html").is_file()) else {
        return api;
    };
    let index = root.join("index.html");
    let assets = Router::new()
        .fallback_service(ServeDir::new(root.join("assets")))
        .layer(middleware::map_response(cache_asset));
    let application = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static(REVALIDATE_CACHE),
        ))
        .service(ServeDir::new(root).fallback(ServeFile::new(index)));
    api.route("/api", any(api_not_found))
        .route("/api/{*path}", any(api_not_found))
        .nest("/assets", assets)
        .fallback_service(application)
}

async fn cache_asset(mut response: Response) -> Response {
    let value = if response.status().is_success() {
        IMMUTABLE_CACHE
    } else {
        NO_STORE_CACHE
    };
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(value));
    response
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "api_route_not_found",
                "message": "The requested API route does not exist.",
                "advice": ["Check the API version and route, then retry."]
            }
        })),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        routing::get,
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{IMMUTABLE_CACHE, NO_STORE_CACHE, REVALIDATE_CACHE, with_workbench};

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("optimist-web-{}", Uuid::new_v4()));
            fs::create_dir_all(root.join("assets")).unwrap();
            fs::write(root.join("index.html"), "<html>Optimist SPA</html>").unwrap();
            fs::write(
                root.join("assets/index-abc123.js"),
                "console.log('optimist')",
            )
            .unwrap();
            Self { root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn serves_root_and_nested_routes_with_spa_fallback() {
        let fixture = Fixture::new();
        let app = with_workbench(
            axum::Router::new().route("/api/v1/health", get(|| async { "ok" })),
            Some(&fixture.root),
        );
        for path in ["/", "/projects/A/feedback"] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CACHE_CONTROL], REVALIDATE_CACHE);
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            assert_eq!(&body[..], b"<html>Optimist SPA</html>");
        }
    }

    #[tokio::test]
    async fn caches_hashed_assets_immutably_and_preserves_api_routes() {
        let fixture = Fixture::new();
        let app = with_workbench(
            axum::Router::new().route("/api/v1/health", get(|| async { "ok" })),
            Some(&fixture.root),
        );
        let asset = app
            .clone()
            .oneshot(
                Request::get("/assets/index-abc123.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(asset.status(), StatusCode::OK);
        assert_eq!(asset.headers()[header::CACHE_CONTROL], IMMUTABLE_CACHE);

        let missing_asset = app
            .clone()
            .oneshot(
                Request::get("/assets/missing.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_asset.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            missing_asset.headers()[header::CACHE_CONTROL],
            NO_STORE_CACHE
        );

        let health = app
            .clone()
            .oneshot(Request::get("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);

        for path in ["/api", "/api/v1/does-not-exist"] {
            let missing = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(missing.status(), StatusCode::NOT_FOUND);
            assert_eq!(missing.headers()[header::CONTENT_TYPE], "application/json");
        }
    }

    #[tokio::test]
    async fn leaves_router_api_only_without_a_valid_build() {
        let app = with_workbench(axum::Router::new(), Some(Path::new("missing-build")));
        let response = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
