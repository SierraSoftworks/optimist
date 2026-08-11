//! Makes sure the workbench build directory exists before it is embedded.
//!
//! A release build compiles `workbench/dist` into the binary. That directory is
//! produced by the frontend toolchain and is not in version control, so a clean
//! checkout has nothing to embed and the build would fail on a missing path
//! rather than on anything a reader could act on.
//!
//! Creating it empty keeps `cargo build --release` working for someone who has
//! no interest in the frontend. The binary then reports that it is serving the
//! API alone, which is a true statement about what was built rather than a
//! compile error about a directory.

use std::{fs, path::PathBuf};

/// What the window build embeds in place of the workbench.
///
/// Tauri wants a frontend to compile into the binary, and the binary already
/// carries one for the server to serve. This stands in its place so that the
/// same files are not embedded twice, and is replaced at startup by the copy
/// the server uses. Seeing it means that replacement did not happen.
const PLACEHOLDER: &str = concat!(
    "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">",
    "<title>Optimist</title></head><body>The workbench was not built.</body></html>\n"
);

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist = root.join("workbench/dist");
    if !dist.is_dir() {
        let _ = fs::create_dir_all(&dist);
    }
    // Rebuild when the frontend does, so a fresh `npm run build` reaches the
    // binary without a `cargo clean`.
    println!("cargo:rerun-if-changed=workbench/dist");
    desktop(&root);
}

/// Generates what the window build needs from `tauri.conf.json`.
#[cfg(feature = "desktop")]
fn desktop(root: &std::path::Path) {
    let placeholder = root.join("gen/frontend");
    fs::create_dir_all(&placeholder).expect("a directory for the placeholder frontend");
    fs::write(placeholder.join("index.html"), PLACEHOLDER).expect("the placeholder frontend");
    tauri_build::build();
}

#[cfg(not(feature = "desktop"))]
fn desktop(_root: &std::path::Path) {
    let _ = PLACEHOLDER;
}
