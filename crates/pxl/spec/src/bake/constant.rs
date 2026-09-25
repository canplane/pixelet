// pxl-spec/bake/constant.rs

use crate::space::adapter::epsg3857::{U_PER_M, Z_SCALE};
use crate::space::hierarchy::Scale;
use crate::space::indexer::{L2, L3};
use crate::space::sparse_tree::mem::SparseTreeLayout;

// ---- Unit System ----

/// UNIT DEFINITIONS:
/// - [u]    : Canonical world unit.
/// - [cell] : Discrete raster unit.
/// - [m]    : Physical meters (EPSG:3857 based).
///
/// SCALE HIERARCHY:
/// - CELL is the cell's side against the unit: 1 cell = CELL.side() u.
/// - at `Scale::of(-1)`, 1u = 2cell, and terrain is baked and drawn at 0.5u.
///
/// It is the raster's resolution and nothing else, so it is a root of its
/// own beside the tiers (space/indexer.rs) rather than one of them: a page
/// and a chunk stay the L3 and L2 blocks whatever it is, and how many cells
/// stand across one is counted from the two. Everything below is derived
/// from it rather than written out, so changing it rescales the page and
/// chunk cell counts, the object ladder and the band's cells together.
pub const CELL: Scale = Scale::of(0);

pub const U_PER_CELL: f64 = CELL.side();

pub const XY_CELL_PER_M: f64 = U_PER_M / U_PER_CELL;

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

// ---- Elevation (Sampling Contract) ----

// ELEVATION SAMPLING CONTRACT:
//
// 1. CENTER-SAMPLE CONVENTION:
//    Both DB-side resampling (SQL) and Client-side reconstruction (Bicubic)
//    MUST use cell-center coordinates (x + 0.5, y + 0.5).
//
// 2. INTERPOLATION MAPPING:
//    Source grid coordinates [sx, sy] are derived from texel [tx, ty] via:
//    step = PAGE_CELLS / PAGE_ELE_SPAN (512 / 32 = 16 cells a sample)
//    sx = tx / step + pad - 0.5
//
// 3. QUANTIZATION:
//    Elevation is stored as I16 [cell] units.
//    Renderer must multiply by U_PER_CELL to restore world [u] units.

/// Inner elevation sample count across one L3 page.
///
/// Derivation:
/// - One terrain page spans 2^L3.shamt() [u].
/// - In the EPSG:3857 adapter, physical page width is therefore:
///   2^L3.shamt() * M_PER_U  [m]
/// - With the current mapping, M_PER_U is approximately 2.388 m/u,
///   so one L3 page is approximately 2^(L3.shamt() + 1) meters wide.
/// - Elevation source resolution is approximately 30 meters.
/// - We approximate that as 32 meters = 2^5 meters for power-of-two alignment.
///
/// So the per-page inner sample span is approximated as:
///   2^(L3.shamt() + 1) / 2^5
/// = 2^(L3.shamt() + 1 - 5)
///
/// This intentionally tracks source data density rather than oversampling it.
/// Boundary continuity is recovered separately by padded reconstruction
/// (ELE_PAD + bicubic interpolation), not by increasing the inner sample count.
pub const PAGE_ELE_SPAN: u32 = 1 << (L3.shamt() + 1 - 5);

/// Two-cell padding (ELE_PAD=2) enables C1 continuous Bicubic interpolation
/// at page boundaries by providing a 4x4 sample kernel for every edge texel.
/// (32 inner + 2*2 padding = 36x36 total grid).
///
/// The pad buys a second thing, and that is why it is a property of the
/// format rather than of the interpolator that reads it: the padded raster
/// reaches ELE_PAD samples — 2 * (PAGE_CELLS / PAGE_ELE_SPAN) = 32 cells
/// — past the page's own span, so the ground height of a cell outside the
/// page is readable from inside it. Whatever has to know what the ground
/// beyond its own span does can ask without the neighbouring page being
/// resident: a mesh deciding whether the cell past its edge rises above the
/// face it is about to draw, a raycast stepping off the page. Reconstruction
/// is the reason the pad is two samples wide; this is the reason it is worth
/// carrying in every page rather than recomputed from neighbours.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;

/// How many sizes an object places at: a cell and its halvings, each tier a
/// level of the object layout below.
///
/// 4 deliberately, not 3. When the cell changed U_PER_CELL from 0.5u to 1u,
/// sliding a fixed count of tiers down from it would have silently dropped
/// the finest one (0.125u) that building at sub-block detail already
/// depended on, instead of gaining a coarser option to match the new cell
/// size. One more halving keeps every size that already worked.
pub const OBJECT_TIERS: usize = 4;

/// The finest tier's side: the cell halved once a tier below it.
pub const OBJECT_FINEST: Scale = CELL.down(OBJECT_TIERS as u32 - 1);

/// A band: a column's chunk stood up as a cube. A column is the L2 block's
/// footprint and its objects are cut into bands along Z, one octree each, and
/// an octree's root is a cube, so a band is as tall as a column is wide —
/// the L2 block's side, in whatever cells the raster is cut into.
pub const BAND: Scale = L2;

/// A band's octree: from the finest tier to the band. Objects never place
/// coarser than a cell, but a tree folds a uniform block up to its root, so
/// a leaf may stand for a whole band (store.rs splits it back into cells).
pub const OBJECT_LAYOUT: SparseTreeLayout = SparseTreeLayout {
    dim: 3,
    min: OBJECT_FINEST,
    max: BAND,
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

/// Cells along a band's side, and as a shift: a band-local column is
/// `(cy << BAND_CELL_SHAMT) | cx`.
pub const BAND_CELLS: u32 = CELL.count_in(BAND);
pub const BAND_CELL_SHAMT: u32 = BAND.above(CELL) as u32;
/// Finest-tier steps along one side of a cell, and one such step in
/// world units.
pub const CELL_STEPS: u32 = OBJECT_FINEST.count_in(CELL);
pub const STEP_UNITS: f64 = OBJECT_FINEST.side();
/// One band's side, in world units.
pub const BAND_UNITS: f64 = BAND.side();
/// An entry's packed position: an axis in finest-tier steps across a band,
/// then its tier above the finest in the bits left over.
pub const POS_BITS: u32 = BAND.above(OBJECT_FINEST) as u32;
pub const POS_MASK: u32 = (1 << POS_BITS) - 1;

/// An entry's packed position: x, y and z in finest-tier steps from its
/// band's corner, and its scale as the tiers above the finest. The one
/// encoding the store writes and the paint and the fold read.
pub const fn pack_pos(x: u32, y: u32, z: u32, scale: Scale) -> u32 {
    x | (y << POS_BITS) | (z << (2 * POS_BITS)) | ((scale.above(OBJECT_FINEST) as u32) << (3 * POS_BITS))
}

/// `pack_pos`'s parts.
pub const fn unpack_pos(pos: u32) -> (u32, u32, u32, Scale) {
    (
        pos & POS_MASK,
        (pos >> POS_BITS) & POS_MASK,
        (pos >> (2 * POS_BITS)) & POS_MASK,
        OBJECT_FINEST.up((pos >> (3 * POS_BITS)) as i32),
    )
}

// An entry is at most a cell, so its tier above the finest is below
// OBJECT_TIERS, and it has to fit in what three axes leave of the word.
const _: () = assert!(3 * POS_BITS < 32 && OBJECT_TIERS <= 1 << (32 - 3 * POS_BITS), "an entry's tier fits beside its position");

#[cfg(test)]
#[path = "constant.test.rs"]
mod test;
