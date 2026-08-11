//! The frontend the window shows, which is the one the server serves.
//!
//! Tauri compiles a frontend into the binary, and so does the server. Embedding
//! the workbench twice would put several megabytes of the same files in one
//! executable, so Tauri is given a placeholder to compile in and handed the real
//! thing here, at startup, out of the copy the server already carries.

use std::borrow::Cow;

use tauri::{
    Assets, Wry,
    utils::assets::{AssetKey, AssetsIter, CspHash},
};

use crate::api::web;

/// The workbench, as Tauri asks for it.
pub(super) struct Workbench(web::Assets);

impl Workbench {
    /// Reads the frontend from a build directory, or from wherever the server
    /// would have read it.
    pub(super) fn new(root: Option<std::path::PathBuf>) -> Self {
        Self(web::Assets::new(root))
    }
}

impl Assets<Wry> for Workbench {
    fn get(&self, key: &AssetKey) -> Option<Cow<'_, [u8]>> {
        web::page(&self.0, key.as_ref().trim_start_matches('/'))
    }

    /// Nothing asks what the frontend contains, only for one file at a time.
    fn iter(&self) -> Box<AssetsIter<'_>> {
        Box::new(std::iter::empty())
    }

    /// The workbench has no inline scripts for a policy to have to name.
    fn csp_hashes(&self, _html_path: &AssetKey) -> Box<dyn Iterator<Item = CspHash<'_>> + '_> {
        Box::new(std::iter::empty())
    }
}

#[cfg(test)]
mod tests {
    use crate::desktop::tests::scratch;

    use super::*;

    fn built() -> (Workbench, crate::desktop::tests::Scratch) {
        let root = scratch();
        std::fs::create_dir_all(root.path().join("assets")).expect("a build directory");
        std::fs::write(root.path().join("index.html"), b"<html>the workbench</html>")
            .expect("a page");
        std::fs::write(root.path().join("assets/index.js"), b"console.log(1)").expect("a script");
        (Workbench::new(Some(root.path().to_path_buf())), root)
    }

    fn asked(workbench: &Workbench, path: &str) -> Option<String> {
        workbench
            .get(&path.into())
            .map(|body| String::from_utf8_lossy(&body).into_owned())
    }

    #[test]
    fn reads_what_the_window_asks_for() {
        let (workbench, _root) = built();

        assert_eq!(asked(&workbench, "/index.html").as_deref(), Some("<html>the workbench</html>"));
        assert_eq!(asked(&workbench, "/assets/index.js").as_deref(), Some("console.log(1)"));
    }

    /// The workbench routes in the window, so a route is the page.
    #[test]
    fn answers_a_route_with_the_page() {
        let (workbench, _root) = built();

        assert_eq!(
            asked(&workbench, "/designs/checkout").as_deref(),
            Some("<html>the workbench</html>")
        );
    }

    /// A missing file answered with the page hides a broken asset reference.
    #[test]
    fn refuses_a_file_that_is_not_there() {
        let (workbench, _root) = built();

        assert_eq!(asked(&workbench, "/assets/missing.js"), None);
    }
}
