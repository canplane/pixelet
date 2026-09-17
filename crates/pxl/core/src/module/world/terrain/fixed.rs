// pxl_core/module/world/terrain/fixed.rs

//! `.fixed.terrain`: where a resident terrain page lies, decided by its own
//! coordinate and not by an allocator.
//!
//! The first and so far only range of the kernel's `.fixed` section
//! (pxl_core::memory), which is what the name says — placed once when the
//! module boots, never freed, and of a size settled at compile time.
//!
//! Two stages, and they know nothing of each other.
//!
//! The index is the page's L3 coordinate reduced modulo the range's axis, the
//! way a clipmap addresses its ring. The axis is the width of the set the load
//! radius reaches — the anchor's own page and KEEP_PAGES either side — so two
//! pages share a record only when they lie a whole axis apart, which is further
//! than the radius reaches. A page arriving therefore overwrites a page that
//! has already left: eviction needs no list, no quarantine and no spare room,
//! and the range is exactly as large as the set it holds.
//!
//! The routing turns a record and a channel into an address, and it is the only
//! thing here that knows how the bytes are ordered. The ground is held channel
//! by channel: a channel's plane is a whole number of mebibytes, so every one of
//! its lanes starts aligned and every row of a lane with it. The elevation is a
//! plane of its own past them, because its record is 9,248 bytes — 32 past a
//! cache line, and a grid whose rows are 136 bytes is not aligned at any offset
//! anyway. Held with the ground it would carry that 32 into every second page's
//! lanes; held apart it costs nothing.

use pxl_shared::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_shared::bake::format::GROUND_CH_KEYS;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};

use super::config_spec::KEEP_DIST;
use crate::render::chunk_range::PAGE_SPAN;

// ---- The index ----

/// How far the load radius reaches from the anchor's own page, in pages.
pub const KEEP_PAGES: u32 = (KEEP_DIST / PAGE_SPAN) as u32;

const _: () = assert!(
    KEEP_PAGES as f64 * PAGE_SPAN == KEEP_DIST,
    "terrain: KEEP_DIST must be a whole number of pages, or the range's axis is not the width of the set it holds"
);

const _: () = assert!(
    crate::module::world::environment::config::render_cfg::CHUNK_RENDER_DIST + crate::render::chunk_range::RANGE_HYSTERESIS <= KEEP_DIST,
    "terrain: a chunk is mounted from a page, so the range chunks are kept to must sit inside the radius pages are loaded from"
);

/// Records along one axis of the range.
pub const TERRAIN_AXIS: u32 = 2 * KEEP_PAGES + 1;

/// Records in the range: exactly the pages the load radius reaches.
pub const TERRAIN_SLOTS: u32 = TERRAIN_AXIS * TERRAIN_AXIS;

/// The record `page_idx` lies in. Every page has one; whether the page is
/// there is the record's own key to say.
pub fn slot_of(page_idx: IVec) -> u32 {
    let x = ivec::X(page_idx).rem_euclid(TERRAIN_AXIS as i64) as u32;
    let y = ivec::Y(page_idx).rem_euclid(TERRAIN_AXIS as i64) as u32;
    y * TERRAIN_AXIS + x
}

// ---- The routing ----

/// One ground channel of a page: a byte a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = PAGE_CELL_SPAN * PAGE_CELL_SPAN;

/// One channel across every record.
pub const GROUND_PLANE_BYTES: u32 = TERRAIN_SLOTS * GROUND_LANE_BYTES;

/// Every channel's plane, one after another.
pub const GROUND_BYTES: u32 = GROUND_CH_KEYS.len() as u32 * GROUND_PLANE_BYTES;

/// A page's elevation: the inner samples and the two-cell skirt bicubic reads
/// at the edges, two bytes a sample.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

/// The elevation across every record.
pub const ELE_PLANE_BYTES: u32 = TERRAIN_SLOTS * ELE_LANE_BYTES;

/// The range, whole: `.fixed.terrain` is this and nothing else.
pub const TERRAIN_BYTES: u32 = GROUND_BYTES + ELE_PLANE_BYTES;

const _: () = assert!(
    GROUND_PLANE_BYTES.is_multiple_of(64),
    "terrain: a ground plane must be whole cache lines, or every second record's lanes start part way into one"
);

/// One page's bytes on their own, channel after channel and the elevation
/// past them: what the shared open-ocean tile is held as, since it belongs to
/// no page and every ocean page reads it.
pub const RECORD_BYTES: u32 = GROUND_CH_KEYS.len() as u32 * GROUND_LANE_BYTES + ELE_LANE_BYTES;

/// Where a standalone record's ground channel lies.
pub fn record_ground_at(record: usize, channel: usize) -> usize {
    record + channel * GROUND_LANE_BYTES as usize
}

/// Where a standalone record's elevation grid lies.
pub fn record_ele_at(record: usize) -> usize {
    record + GROUND_CH_KEYS.len() * GROUND_LANE_BYTES as usize
}

/// Where a record's ground channel lies, `base` the range's own.
pub fn ground_at(base: usize, slot: u32, channel: usize) -> usize {
    base + channel * GROUND_PLANE_BYTES as usize + slot as usize * GROUND_LANE_BYTES as usize
}

/// Where a record's elevation grid lies, `base` the range's own.
pub fn ele_at(base: usize, slot: u32) -> usize {
    base + GROUND_BYTES as usize + slot as usize * ELE_LANE_BYTES as usize
}
