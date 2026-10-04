// pxl-spec/space/indexer.rs
//! The packed level hierarchy over IVec: L4 anchor, L3 page, L2 chunk, L1
//! block (`u`). Lanes are X 24 bits (wraps at the seam), Y 24, Z 16, each
//! signed. Per-level indices shift each lane on its own (`ivec::sh`); a
//! whole-word shift would leak bits across lanes. BASE is inclusive and LIMIT
//! exclusive; a lane narrower than a level collapses there.
// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use super::hierarchy::Scale;
use super::pvf3::ivec::{self, add, fill, IVec};
use super::pvf3::pvf3::{self, Pvf3};
use xpute_core::abi::word::{bit64, field_mask64};

/// Each tier's side against the unit `u` = L1. Every other tier extent steps from these.
pub const L1: Scale = Scale::of(0);
pub const L2: Scale = Scale::of(6);
pub const L3: Scale = Scale::of(9);
pub const L4: Scale = Scale::of(12);
pub const L2_RANGE: u64 = L1.count_in(L2) as u64;
pub const L3_RANGE: u64 = L1.count_in(L3) as u64;
pub const L4_RANGE: u64 = L1.count_in(L4) as u64;

pub const L2_SPAN_IN_L3: u64 = L2.count_in(L3) as u64;
pub const L2_SPAN_IN_L4: u64 = L2.count_in(L4) as u64;
pub const L3_SPAN_IN_L4: u64 = L3.count_in(L4) as u64;
/// `ivec::fill` is not const, so each is the word it evaluates to, pinned by a test against `fill`.
pub const L2_MASK: IVec = fill_const(field_mask64(L2.shamt() as u32, 0));
pub const L3_MASK: IVec = fill_const(field_mask64(L3.shamt() as u32, 0));
pub const L4_MASK: IVec = fill_const(field_mask64(L4.shamt() as u32, 0));

const fn fill_const(e: u64) -> IVec {
    ((e & 0xff_ffff) << 40) | ((e & 0xff_ffff) << 16) | (e & 0xffff)
}

pub fn L2_BASE(addr: IVec) -> IVec {
    addr & !L2_MASK
}
pub fn L3_BASE(addr: IVec) -> IVec {
    addr & !L3_MASK
}
pub fn L4_BASE(addr: IVec) -> IVec {
    addr & !L4_MASK
}

pub fn L2_CENTER(addr: IVec) -> IVec {
    L2_BASE(addr) | (fill(L2_RANGE as i64) >> 1)
}
pub fn L3_CENTER(addr: IVec) -> IVec {
    L3_BASE(addr) | (fill(L3_RANGE as i64) >> 1)
}
pub fn L4_CENTER(addr: IVec) -> IVec {
    L4_BASE(addr) | (fill(L4_RANGE as i64) >> 1)
}

/// The center of the `1 << shamt` cell holding `addr`, on all three axes.
/// PVF3 because at shamt 0 the half is fractional.
pub fn SPAN_CENTER(addr: IVec, shamt: u32) -> Pvf3 {
    let base = Pvf3 {
        i: if shamt == 0 { addr } else { addr & !fill(field_mask64(shamt, 0) as i64) },
        f: 0,
    };
    let half = pvf3::from_real(bit64(shamt) as f64 / 2.0);
    pvf3::gather(pvf3::X(&base) + half, pvf3::Y(&base) + half, pvf3::Z(&base) + half)
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
/// The packed local index of a child level within its parent.
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
/// Packed base to per-level index and back.
pub fn L2_IDX(addr: IVec) -> IVec {
    ivec::sh(L2_BASE(addr), -L2.shamt())
}
pub fn L3_IDX(addr: IVec) -> IVec {
    ivec::sh(L3_BASE(addr), -L3.shamt())
}
pub fn L4_IDX(addr: IVec) -> IVec {
    ivec::sh(L4_BASE(addr), -L4.shamt())
}

pub fn FROM_L2_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L2.shamt())
}
pub fn FROM_L3_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L3.shamt())
}
pub fn FROM_L4_IDX(idx: IVec) -> IVec {
    ivec::sh(idx, L4.shamt())
}

#[cfg(test)]
#[path = "indexer.test.rs"]
mod test;
