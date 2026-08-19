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

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist = root.join("workbench/dist");
    if !dist.is_dir() {
        let _ = fs::create_dir_all(&dist);
    }
    // Rebuild when the frontend does, so a fresh `npm run build` reaches the
    // binary without a `cargo clean`.
    println!("cargo:rerun-if-changed=workbench/dist");
    desktop();
}

/// Generates what the window build needs from `tauri.conf.json`.
#[cfg(feature = "desktop")]
fn desktop() {
    tauri_build::build();
}

#[cfg(not(feature = "desktop"))]
fn desktop() {}
