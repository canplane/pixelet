// xpute-tool/lib.rs

//! These are xpute's build tools. `spec` generates, for every language, the
//! code that spec/ describes; `fmt` runs each generated file through the
//! formatter its tree checks it with.

pub mod fmt;
pub mod spec;

use std::path::PathBuf;

/// This is xpute's root directory, three levels above this crate.
pub fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    root.canonicalize().unwrap_or(root)
}
