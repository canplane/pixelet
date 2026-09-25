// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain page lies in, and where
//! a record's bytes are.
//!
//! The first and much the largest of the kernel's `.fixed` ranges
//! (mem/fixed.rs), which is what the name says — placed once when the
//! module boots, never freed, and of a size settled at compile time.
//!
//! How many records there are, and which page is in which, are the
//! pyramid's (world/pyramid.rs `RECORDS`), the same for every layer, and so
//! is the first, `ABSENT`, which every page the bake left as no object reads
//! — the open ocean; what is terrain's is that a record is a room of fixed
//! bytes here.
//!
//! The routing turns a record into an address, and it is the only thing here
//! that knows how the bytes are ordered. A page's ground is one lane of words,
//! the four channels packed into each (pxl-spec bake/pack.rs), and the lane is
//! a whole number of cache lines, so every record's starts aligned and every
//! row of it with them. The elevation is a plane of its own past them, because
//! its record is 2,592 bytes — 32 past a cache line, and a grid whose rows are
//! 72 bytes is not aligned at any offset anyway. Held with the ground it would
//! carry that 32 into every second page's lanes; held apart it costs nothing.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::world::pyramid::RECORDS;

// ---- The routing ----

/// One page's ground: a `GroundCell` a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = PAGE_CELLS * PAGE_CELLS * core::mem::size_of::<GroundCell>() as u32;

/// The ground across every record.
pub const GROUND_BYTES: u32 = RECORDS * GROUND_LANE_BYTES;

/// A page's elevation: the inner samples and the two-cell skirt bicubic reads
/// at the edges, two bytes a sample.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

/// The elevation across every record.
pub const ELE_PLANE_BYTES: u32 = RECORDS * ELE_LANE_BYTES;

/// The range, whole: `.fixed.terrain` is this and nothing else.
pub const TERRAIN_BYTES: u32 = GROUND_BYTES + ELE_PLANE_BYTES;

const _: () = assert!(
    GROUND_LANE_BYTES.is_multiple_of(64),
    "terrain: a ground lane must be whole cache lines, or every second record's lane starts part way into one"
);

/// Where a record's ground lies in the range.
pub const fn ground_off(record: u32) -> u32 {
    record * GROUND_LANE_BYTES
}

/// Where a record's elevation grid lies in the range.
pub const fn ele_off(record: u32) -> u32 {
    GROUND_BYTES + record * ELE_LANE_BYTES
}
