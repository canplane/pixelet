// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain page lies in — how many
//! records there are, and where a record's bytes are.
//!
//! The first and so far only range of the kernel's `.fixed` section
//! (mem/fixed.rs), which is what the name says — placed once when the
//! module boots, never freed, and of a size settled at compile time.
//!
//! How many records there are is here; which page is in which is
//! resident.rs's, and it is a policy rather than arithmetic. The count is the
//! window's rings — a record per page the camera can want at once — so a
//! wanted page always has room, and what is left over is room for pages the
//! camera has left. A wider view wants more of them and leaves less over.
//!
//! The routing turns a record into an address, and it is the only thing here
//! that knows how the bytes are ordered. A page's ground is one lane of words,
//! the four channels packed into each (pxl-spec bake/pack.rs), and the lane is
//! a whole number of cache lines, so every record's starts aligned and every
//! row of it with them. The elevation is a plane of its own past them, because
//! its record is 2,592 bytes — 32 past a cache line, and a grid whose rows are
//! 72 bytes is not aligned at any offset anyway. Held with the ground it would
//! carry that 32 into every second page's lanes; held apart it costs nothing.

use pxl_spec::bake::constant::{ELE_PADDED_SPAN, PAGE_CELL_SPAN};
use pxl_spec::bake::pack::GroundCell;

use crate::layer::terrain::level::{HELD_AXIS, WINDOW_LEVELS};

// ---- The count ----

/// Records a level's ring takes: one group.
const GROUP_SLOTS: u32 = HELD_AXIS * HELD_AXIS;

/// Records in the range: a ring for every level of the window, so every page
/// the widest camera wants at once has room.
pub const TERRAIN_SLOTS: u32 = WINDOW_LEVELS * GROUP_SLOTS;

// ---- The routing ----

/// One page's ground: a word a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = PAGE_CELL_SPAN * PAGE_CELL_SPAN * core::mem::size_of::<GroundCell>() as u32;

/// The ground across every record.
pub const GROUND_BYTES: u32 = TERRAIN_SLOTS * GROUND_LANE_BYTES;

/// A page's elevation: the inner samples and the two-cell skirt bicubic reads
/// at the edges, two bytes a sample.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

/// The elevation across every record.
pub const ELE_PLANE_BYTES: u32 = TERRAIN_SLOTS * ELE_LANE_BYTES;

/// The range, whole: `.fixed.terrain` is this and nothing else.
pub const TERRAIN_BYTES: u32 = GROUND_BYTES + ELE_PLANE_BYTES;

const _: () = assert!(
    GROUND_LANE_BYTES.is_multiple_of(64),
    "terrain: a ground lane must be whole cache lines, or every second record's lane starts part way into one"
);

/// One page's bytes on their own, channel after channel and the elevation
/// past them: what the shared open-ocean tile is held as, since it belongs to
/// no page and every ocean page reads it.
pub const RECORD_BYTES: u32 = GROUND_LANE_BYTES + ELE_LANE_BYTES;

/// Where a standalone record's ground lies.
pub fn record_ground_at(record: usize) -> usize {
    record
}

/// Where a standalone record's elevation grid lies.
pub fn record_ele_at(record: usize) -> usize {
    record + GROUND_LANE_BYTES as usize
}

/// Where a record's ground lies, `base` the range's own.
pub fn ground_at(base: usize, slot: u32) -> usize {
    base + slot as usize * GROUND_LANE_BYTES as usize
}

/// Where a record's elevation grid lies, `base` the range's own.
pub fn ele_at(base: usize, slot: u32) -> usize {
    base + GROUND_BYTES as usize + slot as usize * ELE_LANE_BYTES as usize
}
