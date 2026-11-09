// pxl-spec/directory.rs

//! The project's folder for what a run keeps or leaves: `tmp/` at the root of the checkout
//! the code was built from, which git ignores (AGENTS.md). Everything a tool caches, and every
//! temporary folder it makes, lies there, where it is seen and cleared as one.

use std::path::{Path, PathBuf};

/// `of` under the checkout's `tmp/`, the root being rust/.cargo's `PXL_REPO`. What a run makes
/// only for itself it removes when it ends.
pub fn tmp_dir(of: &str) -> PathBuf {
    Path::new(env!("PXL_REPO")).join("tmp").join(of)
}
