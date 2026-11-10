// xpute-tool/lib.rs

//! xpute's own build tool: `spec` generates, with xpute-gen, every file that
//! spec/ describes.

pub mod spec;

use std::path::PathBuf;

/// This is xpute's root directory, three levels above this crate.
pub fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    root.canonicalize().unwrap_or(root)
}
