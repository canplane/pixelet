// pxl-tool/lib.rs

pub mod asset;
pub mod fetch;
pub mod fmt;
pub mod spec;

use std::path::{Path, PathBuf};

/// The tree the tool runs in: the nearest directory up from where it was
/// started that holds this tool's own manifest, `rust/tool/Cargo.toml`
/// (xpute's `rust/Cargo.toml` is a workspace too). Found where it runs, not
/// where it was built: a target directory shared between two checkouts
/// hands each the binary built last, from either.
pub fn root() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    here.ancestors().find(|d| d.join("rust/tool/Cargo.toml").is_file()).map_or(here.clone(), Path::to_path_buf)
}
