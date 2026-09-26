// pxl-tool/lib.rs

//! What the repository runs to make the files it ships and the source it
//! generates: the code written from `spec/`, the asset ingest, and the bakes
//! that write a format the tree owns. Each is a binary under `bin/`, run by a
//! task in the root deno.jsonc; what they share is here.

pub mod asset;
pub mod fmt;
pub mod spec;

use std::path::{Path, PathBuf};

/// The repository's root, which every path a tool reads or writes is under.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}
