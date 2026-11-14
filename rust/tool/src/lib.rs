// pxl-tool/lib.rs

pub mod asset;
pub mod brand;
/// pxl-spec's file, compiled here as `directory` is, so an image is decoded to linear light
/// and encoded back by the client's own rule.
#[path = "../../spec/src/color.rs"]
pub mod color;
/// pxl-spec's file, compiled here as well, because the tool depends on no workspace crate
/// (Cargo.toml says why), so the project's folders are still named in one place.
#[path = "../../spec/src/directory.rs"]
pub mod directory;
pub mod download;
pub mod fetch;
/// pxl-spec's file, compiled here as `directory` is, so a model is measured by the same
/// transforms the client places it by.
#[path = "../../spec/src/space/coord/mat4.rs"]
pub mod mat4;
pub mod spec;
/// The vectors `mat4` is written in, pxl-spec's file compiled the same way.
#[path = "../../spec/src/space/coord/v3.rs"]
pub mod v3;

use std::path::{Path, PathBuf};

/// The tree the tool runs in: the nearest directory up from where it was started that holds
/// this tool's own manifest, `rust/tool/Cargo.toml` (xpute's parts hold a `rust/Cargo.toml`
/// too). It is found where the tool runs, not where it was built, because a target directory
/// shared between two checkouts hands each the binary built last, from either.
pub fn root() -> PathBuf {
    let here = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    here.ancestors()
        .find(|d| d.join("rust/tool/Cargo.toml").is_file())
        .map_or(here.clone(), Path::to_path_buf)
}
