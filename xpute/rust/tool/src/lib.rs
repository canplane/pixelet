// xpute-tool/lib.rs

//! xpute's tools: the files generated from spec/ (spec.rs), and the
//! formatters they are written through (fmt.rs).

pub mod fmt;
pub mod spec;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// The repository every path is read and written under, set once by a
/// repository that runs the generators over its own spec.
pub fn set_root(root: PathBuf) {
    let _ = ROOT.set(root);
}

/// xpute's root, two above this crate, where no other was set.
pub fn root() -> &'static Path {
    ROOT.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        root.canonicalize().unwrap_or(root)
    })
}
