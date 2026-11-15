// xpute-tool/lib.rs

//! xpute's own build tool: `spec` generates, with xpute-gen and
//! xpute-link-gen, every file xpute's JSON and WIT describe.

pub mod spec;

use std::path::PathBuf;

/// This is xpute's root directory, one level above this crate.
pub fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    root.canonicalize().unwrap_or(root)
}
