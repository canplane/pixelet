// xpute-tool/lib.rs

//! xpute's tools: the files generated from spec/ (spec.rs), and the
//! formatters they are written through (fmt.rs).

pub mod fmt;
pub mod spec;

use std::path::PathBuf;

/// xpute's root, two above this crate.
pub fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}
