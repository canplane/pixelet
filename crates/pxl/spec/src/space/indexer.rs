// pxl-spec/space/indexer.rs
// Multi-level spatial indexing and hierarchy helpers.

//! Multi-level spatial topology and packed hierarchy helpers.
//!
//! This module defines the packed level hierarchy over IVec and provides:
//! - aligned level boundary helpers (BASE / CENTER / LIMIT)
//! - parent-relative local indices (IDX_IN)
//! - packed logical index conversion (IDX / FROM_*_IDX)
//!
//! Hierarchy (dimension-agnostic — a space-axis scale tier, independent of
//! what shape of data any given layer stores at that tier):
//! - L5 (Shard)         : global sharding domain
//! - L4 (Anchor)        : floating-origin anchor domain
//! - L3 (Page)          : network transaction / paging unit
//! - L2 (Chunk)         : render / LOD batching unit
//! - L1 (Block)         : fundamental canonical spatial unit (`u`)
//! - L0 (Sub-block/SVO) : fractional / sub-block layer handled outside this module
//!
//! Packed lane layout (inherited from PVF3 IVec):
//! [ 63 ................. 40 | 39 ................. 16 | 15 ................. 0 ]
//! [      Lane X (24 bits)   |      Lane Y (24 bits)   |      Lane Z (16 bits)   ]
//!
//! Physical coordinate interpretation:
//! - X: signed lane with cyclic seam semantics
//! - Y: signed lane for north-south extent
//! - Z: signed lane for altitude
//!
//! Address / index domain:
//! - `addr` is a packed L1-granularity address in IVec space.
//! - IVec storage is a packed bitfield. Signedness is a lane interpretation rule,
//!   not a storage rule.
//! - IDX / IDX_IN are per-axis coordinates at the named level, computed via
//!   ivec.sh's per-lane shift — never a raw packed-word shift, which leaks
//!   bits across lane boundaries and corrupts every lane but the outermost
//!   one, positive or negative alike. Safe to reuse for spatial arithmetic
//!   (add/sub) directly.
//!
//! Offset rule:
//! - "offset" is reserved for addr diffs only, i.e. differences between two L1 addrs.
//! - Higher-level hierarchy relations use IDX / IDX_IN, not "offset".
//!
//! Axis participation rule:
//! - BASE / LIMIT are computed mechanically from packed masks and ranges.
//! - If a level range reaches or exceeds a lane width, that lane collapses at that
//!   level.
//! - In a collapsed lane, BASE and LIMIT become identical on that axis, so that
//!   axis contributes no partition range above that level.
//!
//! Current consequence:
//! - X / Y remain hierarchical through L5.
//! - Z collapses at L5 (Z_W = 16, L5_SHAMT = 16), so L5 is effectively horizontal.
//!
//! Naming:
//! - RANGE   = absolute coordinate range at that level, measured in canonical `u` units
//! - SPAN_IN = number of child slots / units contained in the parent level
//!
//! Range convention:
//! - BASE is inclusive
//! - LIMIT is exclusive

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use super::pvf3::ivec::{self, add, fill, IVec};
use super::pvf3::pvf3::{self, Pvf3};
use xpute_core::abi::word::{bit64, field_mask64};

// ---- Hierarchy Configuration ----

/// SHAMT is the cumulative packed bit distance from the canonical base unit (`u` = L1).
pub const L2_SHAMT: u32 = 6;
pub const L3_SHAMT: u32 = 9;
pub const L4_SHAMT: u32 = 12;
// pub const L5_SHAMT: u32 = 16;

// ---- Absolute Ranges (in Canonical `u` Units) ----

// canonical base unit: 1u = 1 L1 block
pub const L2_RANGE: u64 = bit64(L2_SHAMT); // 1 << 6  = 64u
pub const L3_RANGE: u64 = bit64(L3_SHAMT); // 1 << 9  = 512u
pub const L4_RANGE: u64 = bit64(L4_SHAMT); // 1 << 12 = 4096u
                                           // pub const L5_RANGE: u64 = bit64(L5_SHAMT); // 1 << 16 = 65536u

// ---- Relative Spans inside Parent Levels ----

pub const L2_SPAN_IN_L3: u64 = bit64(L3_SHAMT - L2_SHAMT);
pub const L2_SPAN_IN_L4: u64 = bit64(L4_SHAMT - L2_SHAMT);
pub const L3_SPAN_IN_L4: u64 = bit64(L4_SHAMT - L3_SHAMT);
// pub const L3_SPAN_IN_L5: u64 = bit64(L5_SHAMT - L3_SHAMT);
// pub const L4_SPAN_IN_L5: u64 = bit64(L5_SHAMT - L4_SHAMT);

// ---- Hierarchy Masks ----

/// These are component-wise packed masks used to isolate lower-level bits.
/// `ivec::fill` is not const, so each is the word it evaluates to, pinned
/// by a test against `fill`.
pub const L2_MASK: IVec = fill_const(field_mask64(L2_SHAMT, 0));
pub const L3_MASK: IVec = fill_const(field_mask64(L3_SHAMT, 0));
pub const L4_MASK: IVec = fill_const(field_mask64(L4_SHAMT, 0));
// pub const L5_MASK: IVec = fill_const(field_mask64(L5_SHAMT)); // Z masks out completely at this level (Z_W = 16)

/// `ivec::fill(e)` for a non-negative `e`, at compile time.
const fn fill_const(e: u64) -> IVec {
    ((e & 0xff_ffff) << 40) | ((e & 0xff_ffff) << 16) | (e & 0xffff)
}

// ---- Level Navigation ----

/// Resolves aligned level boundaries from an absolute packed L1 address.
pub fn L2_BASE(addr: IVec) -> IVec {
    addr & !L2_MASK
}
pub fn L3_BASE(addr: IVec) -> IVec {
    addr & !L3_MASK
}
pub fn L4_BASE(addr: IVec) -> IVec {
    addr & !L4_MASK
}
// pub fn L5_BASE(addr: IVec) -> IVec { addr & !L5_MASK }

pub fn L2_CENTER(addr: IVec) -> IVec {
    L2_BASE(addr) | (fill(L2_RANGE as i64) >> 1)
}
pub fn L3_CENTER(addr: IVec) -> IVec {
    L3_BASE(addr) | (fill(L3_RANGE as i64) >> 1)
}
pub fn L4_CENTER(addr: IVec) -> IVec {
    L4_BASE(addr) | (fill(L4_RANGE as i64) >> 1)
}
// pub fn L5_CENTER(addr: IVec) -> IVec { L5_BASE(addr) | (fill(L5_RANGE as i64) >> 1) }

/// The centre of the cell that contains `addr`, for a span of `1 << shamt`
/// canonical units — the parameterised form of the three CENTER helpers
/// above, for a span resolved at runtime (a render tier, say) rather than
/// named at compile time.
///
/// PVF3 rather than IVec, because the half is sub-integer at the base level:
/// a 1u cell's centre is 0.5u and lives in the fractional lane. L2/L3/L4 can
/// OR their half into the integer lane only because their spans are 64u and
/// up; this is the same expression written where it also holds at shamt 0.
///
/// All three axes, symmetric. A cell is a cube and its centre is its centre —
/// X/Y quantised while Z is left raw (or the reverse) is two coordinate
/// systems in one point, and everything that marks a cell here already avoids
/// it: a cell's anchor centre adds the half on every axis, and hover, placed
/// cubes and the location puck all read a cell the same way.
pub fn SPAN_CENTER(out: &mut Pvf3, addr: IVec, shamt: u32) -> &mut Pvf3 {
    out.i = if shamt == 0 { addr } else { addr & !fill(field_mask64(shamt, 0) as i64) };
    out.f = 0;
    let half = pvf3::from_real(bit64(shamt) as f64 / 2.0);
    let (x, y, z) = (pvf3::X(out) + half, pvf3::Y(out) + half, pvf3::Z(out) + half);
    pvf3::gather(out, x, y, z)
}

pub fn L2_LIMIT(addr: IVec) -> IVec {
    add(L2_BASE(addr), fill(L2_RANGE as i64))
}
pub fn L3_LIMIT(addr: IVec) -> IVec {
    add(L3_BASE(addr), fill(L3_RANGE as i64))
}
pub fn L4_LIMIT(addr: IVec) -> IVec {
    add(L4_BASE(addr), fill(L4_RANGE as i64))
}
// pub fn L5_LIMIT(addr: IVec) -> IVec { add(L5_BASE(addr), fill(L5_RANGE as i64)) }

// ---- Parent-relative Local Indexing ----

/// Returns the raw packed local index of a child level within its parent level.
///
/// Semantics:
/// - IDX_IN is a logical local key, derived by isolating the bits that
///   participate inside the parent level.
pub fn L1_IDX_IN_L2(addr: IVec) -> IVec {
    addr & L2_MASK
}
pub fn L1_IDX_IN_L3(addr: IVec) -> IVec {
    addr & L3_MASK
}
pub fn L2_IDX_IN_L3(addr: IVec) -> IVec {
    addr & (L3_MASK & !L2_MASK)
}
pub fn L1_IDX_IN_L4(addr: IVec) -> IVec {
    addr & L4_MASK
}
pub fn L2_IDX_IN_L4(addr: IVec) -> IVec {
    addr & (L4_MASK & !L2_MASK)
}
pub fn L3_IDX_IN_L4(addr: IVec) -> IVec {
    addr & (L4_MASK & !L3_MASK)
}
// pub fn L3_IDX_IN_L5(addr: IVec) -> IVec { addr & (L5_MASK & !L3_MASK) }
// pub fn L4_IDX_IN_L5(addr: IVec) -> IVec { addr & (L5_MASK & !L4_MASK) }

// ---- Global Index Conversion ----

/// Converts between aligned packed level bases and logical indices.
///
/// Semantics:
/// - BASE(addr) first clears lower-level bits; ivec.sh then shifts each lane
///   independently, never the whole packed word, so no lane leaks bits into
///   its neighbor.
/// - FROM_*_IDX restores packed base form from that logical index field.
pub fn L2_IDX(addr: IVec) -> IVec {
    ivec::sh(L2_BASE(addr), -(L2_SHAMT as i32))
}
pub fn L3_IDX(addr: IVec) -> IVec {
    ivec::sh(L3_BASE(addr), -(L3_SHAMT as i32))
}
pub fn L4_IDX(addr: IVec) -> IVec {
    ivec::sh(L4_BASE(addr), -(L4_SHAMT as i32))
}
// pub fn L5_IDX(addr: IVec) -> IVec { ivec::sh(L5_BASE(addr), -(L5_SHAMT as i32)) } // Z has collapsed at L5

pub fn FROM_L2_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L2_SHAMT as i32)
}
pub fn FROM_L3_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L3_SHAMT as i32)
}
pub fn FROM_L4_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L4_SHAMT as i32)
}
// pub fn FROM_L5_IDX(idx: IVec) -> IVec { ivec::sh(idx, L5_SHAMT as i32) }

#[cfg(test)]
#[path = "indexer.test.rs"]
mod test;
