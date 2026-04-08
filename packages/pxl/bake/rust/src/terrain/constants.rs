// @pxl/bake/terrain/constants.rs

use pxl_spec::space::adapter::epsg3857::{z_scale, U_PER_M};
use pxl_spec::space::indexer::{L2_SHAMT, L3_SHAMT};

// ---- Unit System ----
//
// UNIT DEFINITIONS:
// - [u]    : Canonical world unit.
// - [cell] : Discrete raster unit.
// - [m]    : Physical meters (EPSG:3857 based).
//
// SCALE HIERARCHY:
// - CELL_SHAMT defines the cell<->u relation.
// - when CELL_SHAMT = -2, 1u = 4 cells (U_PER_CELL = 0.25).

pub const CELL_SHAMT: i32 = -2;
const NEG_CELL_SHAMT: u32 = (-CELL_SHAMT) as u32; // 2

/// Canonical world units per raster cell. (= 2^CELL_SHAMT = 0.25)
pub const U_PER_CELL: f64 = 1.0 / (1u32 << NEG_CELL_SHAMT) as f64;
/// Raster cells per canonical world unit. Valid only when CELL_SHAMT <= 0.
const CELL_PER_U: u32 = 1 << NEG_CELL_SHAMT; // 4

/// Horizontal raster cells per EPSG:3857 meter.
pub const XY_CELL_PER_M: f64 = CELL_PER_U as f64 * U_PER_M;

/// Vertical raster cells per physical meter at the owning L3 page.
///
/// Mercator vertical scale is snapped to the owning L3 page base via `z_scale(y_m)`,
/// so all points in the same page share the same Z quantization factor.
pub fn z_cell_per_m(y_m: f64) -> f64 {
    XY_CELL_PER_M * z_scale(y_m)
}

// ---- Grid Layout (Horizontal) ----

/// One L3 Page span in raster cells. (= 1 << (L3_SHAMT - CELL_SHAMT) = 1 << 9 = 512)
pub const PAGE_CELL_SPAN: u32 = 1 << (L3_SHAMT + NEG_CELL_SHAMT);

/// One L2 Chunk span in raster cells. (= 1 << (L2_SHAMT - CELL_SHAMT) = 1 << 7 = 128)
pub const CHUNK_CELL_SPAN: u32 = 1 << (L2_SHAMT + NEG_CELL_SHAMT);

// ---- Elevation (Sampling Contract) ----
//
// 1. CENTER-SAMPLE CONVENTION:
//    DB-side resampling (SQL) and client-side reconstruction (Bicubic) MUST use
//    cell-center coordinates (x + 0.5, y + 0.5).
//
// 2. INTERPOLATION MAPPING:
//    Source grid coordinates [sx, sy] are derived from texel [tx, ty] via:
//    step = PAGE_CELL_SPAN / PAGE_ELE_SPAN (e.g., 32)
//    sx = tx / step + pad - 0.5
//
// 3. QUANTIZATION:
//    Elevation is stored as I16 [cell] units.
//    Renderer must multiply by U_PER_CELL to restore world [u] units.

/// Inner elevation sample count across one L3 page.
///
/// Derivation:
/// - One page spans 2^L3_SHAMT [u].
/// - With M_PER_U ≈ 4.777, one L3 page is approximately 2^(L3_SHAMT + 2) meters wide.
/// - ESA elevation source resolution is approximately 30 meters.
/// - Approximated as 32 meters = 2^5 for power-of-two alignment.
///
/// Per-page inner sample span ≈ 2^(L3_SHAMT + 2) / 2^5 = 2^(L3_SHAMT + 2 - 5) = 16
pub const PAGE_ELE_SPAN: u32 = 1 << (L3_SHAMT + 2 - 5);

/// Two-cell padding enables C1 continuous Bicubic interpolation at page boundaries
/// by providing a 4×4 sample kernel for every edge texel.
/// (16 inner + 2×2 padding = 20×20 total grid)
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

// ---- Sparse Tree ----

pub struct TerrainLayout {
    pub dim: u32,
    pub min: i32,
    pub max: u32,
}

pub const TERRAIN_LAYOUT: TerrainLayout = TerrainLayout {
    dim: 2,
    min: CELL_SHAMT,
    max: L3_SHAMT,
};
