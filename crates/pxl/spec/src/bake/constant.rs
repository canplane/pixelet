// pxl-spec/bake/constant.rs

use crate::space::adapter::epsg3857::{U_PER_M, Z_SCALE};
use crate::space::indexer::{L2_SHAMT, L3_SHAMT};
use crate::space::pvf3::core::IVEC_X_W;
use crate::space::sparse_tree::mem::SparseTreeLayout;

// ---- Unit System ----

/// UNIT DEFINITIONS:
/// - [u]    : Canonical world unit.
/// - [cell] : Discrete raster unit.
/// - [m]    : Physical meters (EPSG:3857 based).
///
/// SCALE HIERARCHY:
/// - CELL_SHAMT defines the cell<->u relation (cell.spec.rs, from cell.json).
/// - when CELL_SHAMT = -1, 1u = 2cell.
pub use super::cell_spec::{CELL_SHAMT, U_PER_CELL};

// Valid when CELL_SHAMT <= 0.
const CELL_PER_U: u32 = 1 << -CELL_SHAMT;

pub const XY_CELL_PER_M: f64 = CELL_PER_U as f64 * U_PER_M;

/// Vertical [cell] units per physical meter at the owning L3 page.
///
/// Semantics:
/// - Input `y_m` is EPSG:3857 planar Y in meters.
/// - Mercator vertical scale is snapped to the owning L3 page base via `Z_SCALE(y_m)`.
/// - All points within the same L3 page therefore share the same Z quantization factor.
///
/// Used for DB/bake-side elevation and feature height quantization so that
/// terrain/object Z contracts match the shared page-constant Mercator rule.
#[allow(non_snake_case)]
pub fn Z_CELL_PER_M(y_m: f64) -> f64 {
    XY_CELL_PER_M * Z_SCALE(y_m)
}

// ---- Grid Layout (Horizontal) ----

/// One L3 page span in raster cells.
pub const PAGE_CELL_SPAN: u32 = 1 << (L3_SHAMT as i32 - CELL_SHAMT);

/// One L2 chunk span in raster cells.
pub const CHUNK_CELL_SPAN: u32 = 1 << (L2_SHAMT as i32 - CELL_SHAMT);

/// The coarsest level the page pyramid is folded to. A level-k page is 2^k
/// leaves on a side, and at level 10 — zoom 5 — one spans about 9° of
/// latitude, across which the page-constant vertical scale is already 12%
/// off: past it Web Mercator on a plane stops being honest, and a coarser
/// view is a different projection over the same tiles rather than another
/// level (issue/20260921-level-step-and-zoom-are-one-number.md). A bake folds
/// up to it and a server answers nothing above it.
pub const PAGE_TOP_LEVEL: u32 = 10;

/// The zoom a level-0 page is, in the 512-pixel tile convention: the X lane
/// is one EPSG:3857 world, so a level has 2^(IVEC_X_W − L3_SHAMT − level)
/// pages across it, which is a zoom's tile count. Level k is zoom
/// LEVEL0_ZOOM − k, and a level-0 cell is the standard 256-pixel z16 pixel
/// exactly (issue/20260921-level-step-and-zoom-are-one-number.md).
pub const LEVEL0_ZOOM: u32 = IVEC_X_W - L3_SHAMT;

pub const fn zoom_of_level(level: u32) -> u32 {
    LEVEL0_ZOOM - level
}

// The identity holds by three constants agreeing, not by one deriving the
// rest: the X lane is one Mercator world (space/adapter/3857.rs), a cell is
// one unit, and a page is 512 cells. Any of them moving breaks the tiling's
// match with the web map convention, so each is held here.
const _: () = {
    assert!(CELL_SHAMT == 0, "a cell is one unit");
    assert!(PAGE_CELL_SPAN == 512, "a page is one 512-pixel tile");
    assert!(LEVEL0_ZOOM == 15, "level 0 is zoom 15 at 512 pixels, z16 at 256");
    assert!(PAGE_TOP_LEVEL <= LEVEL0_ZOOM, "no level coarser than one page across the world");
};

// ---- Elevation (Sampling Contract) ----

// ELEVATION SAMPLING CONTRACT:
//
// 1. CENTER-SAMPLE CONVENTION:
//    Both DB-side resampling (SQL) and Client-side reconstruction (Bicubic)
//    MUST use cell-center coordinates (x + 0.5, y + 0.5).
//
// 2. INTERPOLATION MAPPING:
//    Source grid coordinates [sx, sy] are derived from texel [tx, ty] via:
//    step = PAGE_CELL_SPAN / PAGE_ELE_SPAN (512 / 32 = 16 cells a sample)
//    sx = tx / step + pad - 0.5
//
// 3. QUANTIZATION:
//    Elevation is stored as I16 [cell] units.
//    Renderer must multiply by U_PER_CELL to restore world [u] units.

/// Inner elevation sample count across one L3 page.
///
/// Derivation:
/// - One terrain page spans 2^(L3_SHAMT) [u].
/// - In the EPSG:3857 adapter, physical page width is therefore:
///   2^(L3_SHAMT) * M_PER_U  [m]
/// - With the current mapping, M_PER_U is approximately 2.388 m/u,
///   so one L3 page is approximately 2^(L3_SHAMT + 1) meters wide.
/// - Elevation source resolution is approximately 30 meters.
/// - We approximate that as 32 meters = 2^5 meters for power-of-two alignment.
///
/// So the per-page inner sample span is approximated as:
///   2^(L3_SHAMT + 1) / 2^5
/// = 2^(L3_SHAMT + 1 - 5)
///
/// This intentionally tracks source data density rather than oversampling it.
/// Boundary continuity is recovered separately by padded reconstruction
/// (ELE_PAD + bicubic interpolation), not by increasing the inner sample count.
pub const PAGE_ELE_SPAN: u32 = 1 << (L3_SHAMT + 1 - 5);

/// Two-cell padding (ELE_PAD=2) enables C1 continuous Bicubic interpolation
/// at page boundaries by providing a 4x4 sample kernel for every edge texel.
/// (32 inner + 2*2 padding = 36x36 total grid).
///
/// The pad buys a second thing, and that is why it is a property of the
/// format rather than of the interpolator that reads it: the padded raster
/// reaches ELE_PAD samples — 2 * (PAGE_CELL_SPAN / PAGE_ELE_SPAN) = 32 cells
/// — past the page's own span, so the ground height of a cell outside the
/// page is readable from inside it. Whatever has to know what the ground
/// beyond its own span does can ask without the neighbouring page being
/// resident: a mesh deciding whether the cell past its edge rises above the
/// face it is about to draw, a raycast stepping off the page. Reconstruction
/// is the reason the pad is two samples wide; this is the reason it is worth
/// carrying in every page rather than recomputed from neighbours.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

/// Object sub-block (SVO) octree layout — one instance per occupied L1
/// cell, not one global tree (`resolve`/`set` only ever consume the low
/// `max-min` bits of the key, so a tree only distinguishes position within
/// one `2^(max-min)`-wide span). min/max track CELL_SHAMT so this rescales
/// automatically the same way object/size.ts's OBJECT_SIZES already does.
///
/// dep = max - min = 3 matches OBJECT_SIZES exactly (4 tiers: the root plus
/// 3 halving steps down to U_PER_CELL/8). max = CELL_SHAMT (not L3_SHAMT)
/// because objects never merge past
/// one cell — sparse_tree's own set()/compact() enforce that ceiling
/// structurally, since there's no coarser level for a tree this shallow to
/// ever report.
pub const OBJECT_LAYOUT: SparseTreeLayout = SparseTreeLayout {
    dim: 3,
    min: CELL_SHAMT - 3,
    max: CELL_SHAMT,
};

// ---- An object entry, as a column's lanes hold it ----

/// A cell's Z and a band's, as wide as the coordinate carries them. PVF3
/// gives Z sixteen bits (pvf3/core.rs's IVEC_Z_W), so a cell's Z is an i16 and
/// a band's — that Z over BAND_CELLS — is narrower still. They are i32 here
/// because the arithmetic over them (a band times its cells) leaves the range
/// a Z has, and a type that cannot hold its own product is worse than a wide
/// one: what the width is for is written down rather than enforced.
pub type CellZ = i32;
pub type BandZ = i32;

/// OBJECT_LAYOUT's finest and coarsest levels: an eighth of a cell, and a
/// cell. A band tree reaches six levels above that, to the band's 64 cells.
pub const MIN_LEVEL: i32 = -3;
pub const CELL_LEVEL: i32 = 0;
pub const BAND_SHAMT: u32 = 6;
pub const BAND_CELLS: u32 = 1 << BAND_SHAMT;
/// Finest-tier steps along one side of a cell, and one such step in
/// world units.
pub const CELL_STEPS: u32 = 1 << (-MIN_LEVEL);
pub const STEP_UNITS: f64 = 1.0 / CELL_STEPS as f64;
/// One band's height, in world units.
pub const BAND_UNITS: f64 = BAND_CELLS as f64;
/// An entry's packed position: nine bits an axis in finest-tier steps,
/// then its level above the finest in four.
pub const POS_BITS: u32 = 9;
pub const POS_MASK: u32 = (1 << POS_BITS) - 1;

#[cfg(test)]
#[path = "constant.test.rs"]
mod test;
