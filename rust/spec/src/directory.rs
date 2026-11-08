// pxl-spec/directory.rs

//! The project's folder for what a run keeps or leaves: `tmp/` at the root of the checkout
//! the code was built from, which git ignores (AGENTS.md). Everything a tool caches, and every
//! temporary folder it makes, lies there, where it is seen and cleared as one.

use std::path::{Path, PathBuf};

/// `of` under the checkout's `tmp/`. A crate that compiles this file lies two folders under
/// the root (rust/spec, rust/tool), so the root is found from the crate's own folder. What a
/// run makes only for itself it removes when it ends.
pub fn tmp_dir(of: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lies two folders under the checkout's root")
        .join("tmp")
        .join(of)
}
