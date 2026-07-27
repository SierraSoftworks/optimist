//! Serving the workbench from the same process as the API.
//!
//! # Where the files come from
//!
//! A debug build reads them from disk, so a rebuild of the frontend appears on
//! the next reload without restarting anything. A release build carries them
//! inside the binary, because a released server should be one file rather than a
//! file and a directory that have to be kept together and in step.
//!
//! Either can be overridden by pointing at a build directory, which is what lets
//! somebody serve a frontend they built elsewhere without recompiling.
//!
//! # Why the API cannot fall back
//!
//! Everything the browser asks for that is not a file becomes `index.html`,
//! because the workbench routes in the browser and a deep link has to survive a
//! reload. That rule must never reach `/api`: a mistyped endpoint answering with
//! a page of HTML would be read by a client as a malformed response to a request
//! that it believed had succeeded. Unknown API paths stay JSON errors, and the
//! fallback is mounted so that it never sees them.

use std::{borrow::Cow, path::PathBuf};

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

/// The workbench compiled into the binary.
///
/// Empty in a checkout where the frontend was never built; the build script
/// guarantees the directory exists so that this compiles either way.
#[cfg(not(debug_assertions))]
static EMBEDDED: include_dir::Dir<'static> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/workbench/dist");

/// Where the frontend is read from, in the order it is tried.
#[derive(Clone, Debug)]
pub struct Assets {
    /// A build directory named by the operator, which wins over everything.
    root: Option<PathBuf>,
}

impl Assets {
    /// Chooses where to read the workbench from.
    ///
    /// An explicit directory is used whether or not this is a release build, so
    /// that a packaged server can still be pointed at a newer frontend. Without
    /// one, a debug build looks beside the repository and a release build uses
    /// what it was compiled with.
    pub fn new(root: Option<PathBuf>) -> Self {
        let root = root.or_else(default_root).filter(|path| {
            let present = path.join("index.html").is_file();
            if !present {
                eprintln!(
                    "no workbench build at {}; serving the API alone",
                    path.display()
                );
            }
            present
        });
        Self { root }
    }

    /// Reads one file, by its path within the build.
    fn read(&self, path: &str) -> Option<Cow<'static, [u8]>> {
        if let Some(root) = &self.root {
            // `..` in a request must not climb out of the build directory. Each
            // segment is checked rather than the joined path, because a check
            // after joining can be defeated by a symlink inside the directory.
            if path
                .split('/')
                .any(|part| part == ".." || part == "." || part.is_empty())
            {
                return None;
            }
            return std::fs::read(root.join(path)).ok().map(Cow::Owned);
        }
        #[cfg(not(debug_assertions))]
        {
            return EMBEDDED
                .get_file(path)
                .map(|file| Cow::Borrowed(file.contents()));
        }
        #[cfg(debug_assertions)]
        None
    }

    /// Reports whether there is a workbench to serve at all.
    fn available(&self) -> bool {
        if self.root.is_some() {
            return true;
        }
        #[cfg(not(debug_assertions))]
        {
            return EMBEDDED.get_file("index.html").is_some();
        }
        #[cfg(debug_assertions)]
        false
    }
}

/// Where a debug build looks when nothing was named.
#[cfg(debug_assertions)]
fn default_root() -> Option<PathBuf> {
    Some(PathBuf::from("workbench/dist"))
}

#[cfg(not(debug_assertions))]
fn default_root() -> Option<PathBuf> {
    None
}

/// Adds the workbench to a router, leaving the API in front of it.
pub(super) fn attach(router: Router, assets: Assets) -> Router {
    if !assets.available() {
        return router;
    }
    router.fallback(move |uri: Uri| serve(assets.clone(), uri))
}

async fn serve(assets: Assets, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if let Some(body) = assets.read(path) {
        return file(path, body);
    }

    // A request for something that looks like a file and is not there is a
    // missing file, not a browser route. Answering it with the page would turn
    // a broken asset reference into a silent one.
    if looks_like_a_file(path) {
        return StatusCode::NOT_FOUND.into_response();
    }

    match assets.read("index.html") {
        Some(body) => file("index.html", body),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn looks_like_a_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn file(path: &str, body: Cow<'static, [u8]>) -> Response {
    let mut response = Response::new(Body::from(body.into_owned()));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    // Generated assets carry a content hash in their name, so a given URL never
    // changes and can be kept forever. The page that references them must not
    // be, or a deploy would be invisible until every cache expired.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control(path)),
    );
    response
}

fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// Content types for what a frontend build produces.
///
/// A fixed table rather than a lookup crate: the set is small, it is decided by
/// the bundler rather than by a user, and getting it wrong is visible the moment
/// a page is loaded.
fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_assets_are_kept_and_pages_are_not() {
        assert!(cache_control("assets/index-abc123.js").contains("immutable"));
        assert_eq!(cache_control("index.html"), "no-cache");
    }

    #[test]
    fn a_request_cannot_climb_out_of_the_build_directory() {
        let assets = Assets {
            root: Some(PathBuf::from("workbench/dist")),
        };
        assert!(assets.read("../../Cargo.toml").is_none());
        assert!(assets.read("a/../../Cargo.toml").is_none());
    }

    #[test]
    fn a_path_with_an_extension_is_a_file_and_the_rest_are_routes() {
        assert!(looks_like_a_file("assets/index-abc.js"));
        assert!(looks_like_a_file("favicon.ico"));
        assert!(!looks_like_a_file("d/checkout/design"));
        assert!(!looks_like_a_file(""));
    }

    #[test]
    fn content_types_cover_what_a_build_emits() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(
            content_type("assets/app.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(content_type("assets/app.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("assets/font.woff2"), "font/woff2");
    }
}
