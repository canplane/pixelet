// @pxl/spec/space/indexer.rs
//
// Multi-level spatial indexing and hierarchy helpers.
//
// Hierarchy (coarse to fine):
//   L4 (Anchor) — L3 (Page) — L2 (Chunk) — L1 (Block = 1u canonical unit)
//
// Packed lane layout (inherited from IVec):
// [ 63 ............ 40 | 39 ............ 16 | 15 ........... 0 ]
// [   Lane X (24 bits)  |   Lane Y (24 bits)  |  Lane Z (16 bits) ]
//
// `addr` is a packed L1-granularity IVec address.
// IDX / IDX_IN are unsigned logical keys — do not reuse for signed spatial arithmetic.
//
// BASE is inclusive, LIMIT is exclusive.

use super::pvf3::core::{bit64, field_mask64};
use super::pvf3::ivec::{self, IVec};

// ---- Hierarchy Configuration ----
// SHAMT = cumulative packed bit distance from canonical base unit (L1 = 1u).

pub const L2_SHAMT: u32 = 5;
pub const L3_SHAMT: u32 = 7;
pub const L4_SHAMT: u32 = 12;

// ---- Absolute Ranges (in canonical `u` units) ----

pub const L2_RANGE: u64 = bit64(L2_SHAMT); // 32u
pub const L3_RANGE: u64 = bit64(L3_SHAMT); // 128u
pub const L4_RANGE: u64 = bit64(L4_SHAMT); // 4096u

// ---- Relative Spans inside Parent Levels ----

pub const L2_SPAN_IN_L3: u64 = bit64(L3_SHAMT - L2_SHAMT);
pub const L2_SPAN_IN_L4: u64 = bit64(L4_SHAMT - L2_SHAMT);
pub const L3_SPAN_IN_L4: u64 = bit64(L4_SHAMT - L3_SHAMT);

// ---- Hierarchy Masks ----
// Per-lane packed masks: lower SHAMT bits set in each lane.

pub const L2_MASK: IVec = ivec::fill(field_mask64(L2_SHAMT, 0) as i64);
pub const L3_MASK: IVec = ivec::fill(field_mask64(L3_SHAMT, 0) as i64);
pub const L4_MASK: IVec = ivec::fill(field_mask64(L4_SHAMT, 0) as i64);

// ---- Level Navigation ----

pub fn l2_base(addr: IVec) -> IVec { addr & !L2_MASK }
pub fn l3_base(addr: IVec) -> IVec { addr & !L3_MASK }
pub fn l4_base(addr: IVec) -> IVec { addr & !L4_MASK }

pub fn l2_center(addr: IVec) -> IVec { l2_base(addr) | (ivec::fill(L2_RANGE as i64) >> 1) }
pub fn l3_center(addr: IVec) -> IVec { l3_base(addr) | (ivec::fill(L3_RANGE as i64) >> 1) }
pub fn l4_center(addr: IVec) -> IVec { l4_base(addr) | (ivec::fill(L4_RANGE as i64) >> 1) }

pub fn l2_limit(addr: IVec) -> IVec { ivec::add(l2_base(addr), ivec::fill(L2_RANGE as i64)) }
pub fn l3_limit(addr: IVec) -> IVec { ivec::add(l3_base(addr), ivec::fill(L3_RANGE as i64)) }
pub fn l4_limit(addr: IVec) -> IVec { ivec::add(l4_base(addr), ivec::fill(L4_RANGE as i64)) }

// ---- Parent-relative Local Indexing ----
// IDX_IN: unsigned packed local key within the parent level.

pub fn l1_idx_in_l2(addr: IVec) -> IVec { addr & L2_MASK }
pub fn l1_idx_in_l3(addr: IVec) -> IVec { addr & L3_MASK }
pub fn l2_idx_in_l3(addr: IVec) -> IVec { addr & (L3_MASK & !L2_MASK) }
pub fn l1_idx_in_l4(addr: IVec) -> IVec { addr & L4_MASK }
pub fn l2_idx_in_l4(addr: IVec) -> IVec { addr & (L4_MASK & !L2_MASK) }
pub fn l3_idx_in_l4(addr: IVec) -> IVec { addr & (L4_MASK & !L3_MASK) }

// ---- Global Index Conversion ----
// IDX: registry / hash / routing key derived from aligned packed level base.

pub fn l2_idx(addr: IVec) -> IVec { l2_base(addr) >> L2_SHAMT }
pub fn l3_idx(addr: IVec) -> IVec { l3_base(addr) >> L3_SHAMT }
pub fn l4_idx(addr: IVec) -> IVec { l4_base(addr) >> L4_SHAMT }

pub fn from_l2_idx(idx: IVec) -> IVec { idx << L2_SHAMT }
pub fn from_l3_idx(idx: IVec) -> IVec { idx << L3_SHAMT }
pub fn from_l4_idx(idx: IVec) -> IVec { idx << L4_SHAMT }
