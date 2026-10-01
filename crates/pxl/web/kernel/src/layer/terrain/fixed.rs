// pxl-kernel/layer/terrain/fixed.rs

//! `.fixed.terrain`: the room every resident terrain page lies in, and where
//! a record's bytes are.
//!
//! The first and much the largest of the kernel's `.fixed` ranges
//! (mem/fixed.rs), which is what the name says — placed once when the
//! module boots, never freed, and of a size settled at compile time.
//!
//! How many records there are, and which page is in which, are residency's
//! (world/paging.rs `RECORDS`), the same for every layer, and so
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
//! The index lane (pxl-spec bake/format.rs `Index`) is a third plane past the
//! elevation, a (lowest, highest) pair of i16 a cell of its lattice, and its
//! blocks a fourth: made from the lane as the page is written into its
//! record, and so held exactly as long as the lane is.
//!
//! A page's decks (pxl-spec bake/deck.rs) are six planes between the ground
//! and the elevation, one a lane of theirs, each lane as long as a record
//! holds and a whole number of cache lines: placed past the ground, whose
//! plane ends on a line, every record's deck lanes start on one. A record
//! holds the decks of its page from the front of each lane.

use pxl_spec::bake::constant::ELE_PADDED_SPAN;
use pxl_spec::bake::deck::{MAX_DECKS, MAX_POINTS};
use pxl_spec::bake::format::{Block, BLOCK_LANE};
use pxl_spec::bake::pack::GroundCell;
use pxl_spec::space::pyramid::{EXTENT, PAGE_CELLS};

use crate::world::paging::RECORDS;

// ---- The routing ----

/// One page's ground: a `GroundCell` a cell, no stride and no padding.
pub const GROUND_LANE_BYTES: u32 = PAGE_CELLS * PAGE_CELLS * core::mem::size_of::<GroundCell>() as u32;

/// The ground across every record.
pub const GROUND_BYTES: u32 = RECORDS * GROUND_LANE_BYTES;

/// A deck lane of `count` items of `size` bytes, rounded up to a whole
/// cache line.
const fn deck_lane(count: usize, size: usize) -> u32 {
    ((count * size).div_ceil(64) * 64) as u32
}

/// A page's deck lanes, in the order their planes lie: where each deck's
/// points start, the points, their tops, their piers, each deck's width and
/// its kind.
pub const DECK_LANE_BYTES: [u32; 6] = [
    deck_lane(MAX_DECKS + 1, 4),
    deck_lane(MAX_POINTS, 4),
    deck_lane(MAX_POINTS, 2),
    deck_lane(MAX_POINTS, 1),
    deck_lane(MAX_DECKS, 1),
    deck_lane(MAX_DECKS, 1),
];

/// The deck planes across every record.
pub const DECK_BYTES: u32 = RECORDS * (DECK_LANE_BYTES[0] + DECK_LANE_BYTES[1] + DECK_LANE_BYTES[2] + DECK_LANE_BYTES[3] + DECK_LANE_BYTES[4] + DECK_LANE_BYTES[5]);

/// A page's elevation: the inner samples and the two-cell skirt bicubic reads
/// at the edges, two bytes a sample.
pub const ELE_LANE_BYTES: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN * 2;

/// The elevation across every record.
pub const ELE_PLANE_BYTES: u32 = RECORDS * ELE_LANE_BYTES;

/// A page's index lane: two i16 a cell of the index lattice.
pub const EXTENT_LANE_BYTES: u32 = EXTENT.page_cells() * EXTENT.page_cells() * 4;

/// The index lanes across every record.
pub const EXTENT_PLANE_BYTES: u32 = RECORDS * EXTENT_LANE_BYTES;

/// A page's index blocks, one `Block` each.
pub const BLOCK_LANE_BYTES: u32 = BLOCK_LANE as u32 * core::mem::size_of::<Block>() as u32;

/// The index blocks across every record.
pub const BLOCK_PLANE_BYTES: u32 = RECORDS * BLOCK_LANE_BYTES;

/// The range, whole: `.fixed.terrain` is this and nothing else.
pub const TERRAIN_BYTES: u32 = GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + BLOCK_PLANE_BYTES;

const _: () = assert!(
    GROUND_LANE_BYTES.is_multiple_of(64),
    "terrain: a ground lane must be whole cache lines, or every second record's lane starts part way into one"
);

/// Where a record's ground lies in the range.
pub const fn ground_off(record: u32) -> u32 {
    record * GROUND_LANE_BYTES
}

/// Where a record's deck lane `lane` (`DECK_LANE_BYTES`' order) lies in the
/// range.
pub const fn deck_off(record: u32, lane: usize) -> u32 {
    let mut plane = GROUND_BYTES;
    let mut k = 0;
    while k < lane {
        plane += RECORDS * DECK_LANE_BYTES[k];
        k += 1;
    }
    plane + record * DECK_LANE_BYTES[lane]
}

/// Where a record's elevation grid lies in the range.
pub const fn ele_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + record * ELE_LANE_BYTES
}

/// Where a record's index lane lies in the range.
pub const fn extent_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + record * EXTENT_LANE_BYTES
}

/// Where a record's index blocks lie in the range.
pub const fn blocks_off(record: u32) -> u32 {
    GROUND_BYTES + DECK_BYTES + ELE_PLANE_BYTES + EXTENT_PLANE_BYTES + record * BLOCK_LANE_BYTES
}
