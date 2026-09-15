// pxl_core/lib/env/config.rs

//! Deployment configuration: values that ship as data. The bundler defines
//! them for the shell (`__APP_ENV__`, `virtual:pxl-build`,
//! `__PUBLISHED_TILES__`), and the shell hands them over once at boot.

use super::route::{tile_base, TileBase, TileKind};
use crate::global::Global;

#[derive(Clone, Debug, Default)]
pub struct BuildInfo {
    /// Application environment name, from VITE_APP_ENV (falling back to
    /// Vite's mode). Shown in settings so it is possible to tell which build is
    /// on screen — on a phone there is no URL bar or devtools to check against.
    pub app_env: String,
    /// When this bundle was built, ISO-8601 in UTC.
    pub build_time: String,
    /// Which source this bundle was built from, `r<commits>-<hash>[-dirty]`.
    /// Stamped on every ledger report and trace so a measurement names what it
    /// measured.
    pub build_id: String,
    /// Which official build this is, counted per deploy outside git: `b<n>`,
    /// `b<n>+` for a local build after it, `b?` before any.
    pub build_number: String,
    /// Whether tiles come from a published bake rather than a live API. Decides
    /// whether a version token is in the path and whether a 404 means "never
    /// baked" — open ocean, an empty POI page, ground outside coverage — or a
    /// genuine failure.
    pub published_tiles: bool,
}

static BUILD: Global<BuildInfo> = Global::new();

/// The shell's boot: what the bundler defined.
pub fn set_build_info(info: BuildInfo) {
    BUILD.set(info);
}

fn build() -> &'static BuildInfo {
    BUILD.get(BuildInfo::default)
}

#[allow(non_snake_case)]
pub fn APP_ENV() -> &'static str {
    &build().app_env
}

#[allow(non_snake_case)]
pub fn BUILD_TIME() -> &'static str {
    &build().build_time
}

#[allow(non_snake_case)]
pub fn BUILD_ID() -> &'static str {
    &build().build_id
}

#[allow(non_snake_case)]
pub fn BUILD_NUMBER() -> &'static str {
    &build().build_number
}

#[allow(non_snake_case)]
pub fn PUBLISHED_TILES() -> bool {
    build().published_tiles
}

#[allow(non_snake_case)]
pub fn TILE_BASE_URL() -> TileBase {
    tile_base(TileKind::terrain, PUBLISHED_TILES())
}

#[allow(non_snake_case)]
pub fn POI_TILE_BASE_URL() -> TileBase {
    tile_base(TileKind::poi, PUBLISHED_TILES())
}

#[allow(non_snake_case)]
pub fn GEOCODE_TILE_BASE_URL() -> TileBase {
    tile_base(TileKind::geocode, PUBLISHED_TILES())
}
