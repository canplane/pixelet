// pxl_shared/bake/constant.rs

use crate::spec::space::adapter::epsg3857::{U_PER_M, Z_SCALE};
use crate::spec::space::indexer::{L2_SHAMT, L3_SHAMT};
use crate::spec::space::sparse_tree::mem::SparseTreeLayout;

// ---- Unit System ----

/// UNIT DEFINITIONS:
/// - [u]    : Canonical world unit.
/// - [cell] : Discrete raster unit.
/// - [m]    : Physical meters (EPSG:3857 based).
///
/// SCALE HIERARCHY:
/// - CELL_SHAMT defines the cell<->u relation.
/// - when CELL_SHAMT = -1, 1u = 2cell.
pub const CELL_SHAMT: i32 = 0;
/// `2 ** CELL_SHAMT`, a shift either way.
pub const U_PER_CELL: f64 = if CELL_SHAMT >= 0 { (1u64 << CELL_SHAMT) as f64 } else { 1.0 / (1u64 << -CELL_SHAMT) as f64 };
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

// ---- Elevation (Sampling Contract) ----

// ELEVATION SAMPLING CONTRACT:
//
// 1. CENTER-SAMPLE CONVENTION:
//    Both DB-side resampling (SQL) and Client-side reconstruction (Bicubic)
//    MUST use cell-center coordinates (x + 0.5, y + 0.5).
//
// 2. INTERPOLATION MAPPING:
//    Source grid coordinates [sx, sy] are derived from texel [tx, ty] via:
//    step = PAGE_CELL_SPAN / PAGE_ELE_SPAN (1024 / 64 = 16 cells a sample)
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
/// (64 inner + 2*2 padding = 68x68 total grid).
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

// ---- Sparse Tree ----

// TERRAIN_LAYOUT — orphaned, no consumers anywhere in the repo. Terrain
// currently stores/serves dense arrays via bake/format.rs's
// encode_terrain/decode_terrain + gzip; moving it onto sparse_tree isn't
// recommended, since terrain data is dense/varied rather than
// sparse/mergeable like objects, so a tree would likely add overhead
// without a clear payoff. Commented out rather than deleted in case that
// direction is ever revisited.
// pub const TERRAIN_LAYOUT: SparseTreeLayout = SparseTreeLayout {
//     dim: 2,
//
//     min: CELL_SHAMT,
//     max: L3_SHAMT as i32,
// };

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

#[cfg(test)]
#[path = "constant.test.rs"]
mod test;
