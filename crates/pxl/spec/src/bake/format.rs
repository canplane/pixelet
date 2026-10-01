// pxl-spec/bake/format.rs

//! Terrain format helpers for baked page payloads.
//!
//! The wire field order is defined here.
//!
//! Layer model:
//! - L3 page = terrain transport / storage unit
//! - terrain = static terrain layer carried by that page
//! - ground  = categorical 2D terrain channels
//! - ele     = elevation raster payload for that terrain layer
//!
//! In other words:
//!   L3 page
//!     -> terrain layer
//!          -> ground
//!          -> ele
//!
//! raw wire   = [ele, buildings, segments, base, coasts, decks]
//!   buildings = [[gers_id, subtype, class, height, num_floors, geom_wkb], ...]
//!   segments  = [[gers_id, subtype, class, flags, widths, access, geom_wkb], ...]
//!   base      = [[gers_id, source, subtype, class, geom_wkb], ...]  (source: land_use|water|land|infrastructure)
//!   coasts    = [[id, geom_wkb, covers_page], ...]
//!   decks     = [[gers_id, subtype, class, width_m, piers, geom_wkb], ...]  (LineStringZ, the top in m)
//!
//! baked wire = [ele, cells, extent, occupancy, decks]
//!   cells     = one u16 a cell, the four ground channels packed into it
//!               (bake/pack.rs), which is the shape the client holds too
//!   extent    = the index lane's (lowest, highest) a cell, in `EXTENT`'s
//!               cell order, flattened to i16 pairs (`Index`)
//!   occupancy = one u8: which of the page and its four children are pages
//!               (`Occupancy`)
//!   decks     = the ways the page carries above its ground (bake/deck.rs)

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::constant::ELE_PADDED_SPAN;
use super::deck::{self, Decks};
use super::pack::{channel_of, GroundCell};
use super::surface::extent_of;
use crate::space::pyramid::{EXTENT, PAGE_CELLS};

/// A page's lanes, as a count of what each holds: a page is these and no
/// other size.
pub const GROUND_LANE: usize = (PAGE_CELLS * PAGE_CELLS) as usize;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
pub const EXTENT_LANE: usize = (EXTENT.page_cells() * EXTENT.page_cells()) as usize;
pub const BLOCK_LANE: usize = EXTENT.page_blocks() as usize;

// ---- DB ----

pub struct BuildingRow {
    pub gers_id: String,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub height: Option<f64>,
    pub num_floors: Option<f64>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub struct SegmentRow {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    /// Raw JSON text of the segment's `road_flags` or `rail_flags`, whichever
    /// its subtype carries.
    pub flags: Option<String>,
    /// Raw JSON text of its `width_rules`.
    pub widths: Option<String>,
    /// Raw JSON text of its `access_restrictions`.
    pub access: Option<String>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub struct BaseRow {
    pub gers_id: String,
    pub source: String, // "land_use" | "water" | "land" | "infrastructure"
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>, // WKB
}

/// A deck as the offline solve left it (pxl-geo-bake deck/): its way's
/// subtype, class and width in metres, its line with the deck's top in
/// metres as Z, and a byte a point, 1 where a pier stands under it.
pub struct DeckRow {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub width_m: f64,
    pub piers: Box<[u8]>,
    pub geom_wkb: Box<[u8]>, // WKB, LineString Z
}

pub struct CoastRow {
    pub id: String,
    pub geom_wkb: Box<[u8]>, // WKB
    pub covers_page: bool,
}

// ---- Ground ----

pub use crate::cell::cube_type::GroundChannelKey;

/// The one order the ground channels are in: the lanes of a baked page, the
/// planes of `.fixed.terrain`, and the order a cell is painted in, a later
/// channel's non-void type winning. Every consumer reads it through
/// `ch_index`, so moving a channel is this line and a re-bake.
///
/// Structure paints last: a building that overlaps a road keeps its cells.
/// The overlap is about one cell in a hundred in a dense page, and a road that
/// loses its rim is still a line, while a small building that loses its
/// street-facing cells can stop existing
/// (issue/20260921-level-step-and-zoom-are-one-number.md).
pub const GROUND_CH_KEYS: [GroundChannelKey; 4] = [GroundChannelKey::LAND, GroundChannelKey::WATER, GroundChannelKey::TRANSPORT, GroundChannelKey::STRUCTURE];

/// Where `key` sits in `GROUND_CH_KEYS`.
pub const fn ch_index(key: GroundChannelKey) -> usize {
    let mut i = 0;
    while i < GROUND_CH_KEYS.len() {
        if GROUND_CH_KEYS[i] as u8 == key as u8 {
            return i;
        }
        i += 1;
    }
    panic!()
}

const _: () = {
    let mut id = 0;
    while id < GROUND_CH_KEYS.len() as u32 {
        match GroundChannelKey::from_id(id) {
            Some(k) => assert!(GROUND_CH_KEYS[ch_index(k)] as u8 == k as u8),
            None => panic!("an identity with no channel"),
        }
        id += 1;
    }
};

/// A page's ground: one word a cell, the four channels packed into it
/// (`bake/pack.rs`). A decoded page owns its cells; a resident one reads the
/// block it was written into.
///
/// This is the one shape ground has. The bake paints into it, the wire
/// carries it, the client holds it and the fold reads and writes it, so
/// nothing between a page being baked and the ground being drawn from it
/// converts a representation
/// (issue/20260923-a-ground-cell-is-fourteen-bits.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: std::borrow::Cow<'static, [GroundCell]>,
}

impl Ground {
    /// The catalog number channel `key` holds at cell `i`, as a plane held it.
    pub fn channel(&self, i: usize, key: GroundChannelKey) -> u8 {
        channel_of(self.cells[i], key)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Target ground channel + cell type resolved by a provider's own policy for one normalized feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellConfig {
    pub cell_type: u8,
    pub ch_key: GroundChannelKey,
}

// ---- Index ----

/// Which of a page and its four children are pages of their own rather than
/// the zero page — the open ocean every page the bake left as no object
/// reads, under which nothing is. The four children's in `slot_of`'s order,
/// the low four bits, and the page's own past them.
///
/// A parent is a page exactly when one of its children is, so the page's own
/// bit says something the children's do not only at the finest level, where
/// there are none: a baked leaf is a page and the ocean is not. Carried by
/// the page, the fold reads it off the children it folds whoever folds them —
/// the bake, which reads the ocean where it has no object, or the client,
/// which holds that page in `ABSENT` — and a client never asks for a child
/// its parent says is the ocean.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occupancy(u8);

impl Occupancy {
    const PAGE: u8 = 1 << 4;

    /// The zero page's.
    pub const ZERO: Occupancy = Occupancy(0);

    /// A baked page of the finest level's: a page, with no children.
    pub const LEAF: Occupancy = Occupancy(Occupancy::PAGE);

    /// A parent's, from its four children's.
    pub fn of_children(children: [Occupancy; 4]) -> Occupancy {
        let bits = (0..4).filter(|&q| children[q].is_page()).fold(0, |bits, q| bits | 1 << q);
        Occupancy(if bits == 0 { 0 } else { Occupancy::PAGE | bits })
    }

    /// Whether the page is one of its own, not the zero page.
    pub fn is_page(self) -> bool {
        self.0 & Occupancy::PAGE != 0
    }

    /// Whether the child in `slot` is.
    pub fn has_child(self, slot: usize) -> bool {
        self.0 & (1 << slot) != 0
    }

    /// The wire's byte.
    pub fn bits(self) -> u8 {
        self.0
    }

    /// The occupancy a byte names, none for one no page can have: a bit past
    /// the five, or a child without its parent.
    pub fn of_bits(bits: u8) -> Option<Occupancy> {
        (bits == 0 || (bits & Occupancy::PAGE != 0 && bits < Occupancy::PAGE << 1)).then_some(Occupancy(bits))
    }
}

/// A page's index lane: what it says of everything under it, so that what is
/// not held can be reasoned about from what is. Each of the index lattice's
/// cells (`EXTENT`) holds the lowest and highest the ground drawn there
/// stands, in cells, at this page's level and every level under it —
/// conservative, the interpolation's overshoot and the cells' recess
/// included — so a ray that passes over a cell's highest passes over
/// everything finer there, and one under its lowest has hit. The children's
/// occupancy says which of them there is anything to ask for.
///
/// Made from the leaves up by the one who has them: a leaf's from its own
/// surface (`Terrain::leaf`), a parent's from its own and its children's
/// (bake/fold.rs `fold_index`). The client cannot know what lies under what
/// it does not hold, so it makes none it was not given the children of.
///
/// Beside the lane, its blocks: for every aligned square of its cells 2^j
/// a side (pxl-spec space/pyramid.rs `Lattice::block_index`), the lowest
/// and highest of their extents and the widest any one of them spans — the
/// lane summed over every node of the hierarchy a page holds, so what a
/// node says is one read and not a walk over its cells. Made from the lane
/// whenever an index is, never carried on the wire, and nothing changes
/// the lane without making them again.
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    extent: Cow<'static, [[i16; 2]]>,
    blocks: Cow<'static, [Block]>,
    pub occupancy: Occupancy,
}

/// What an aligned block of an index lane's cells says of all of them: the
/// lowest and highest of their extents, in cells, and the most any one of
/// them spans from its lowest to its highest. Each is a min or a max over
/// the cells, so a block is its four quarters' join, exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub struct Block {
    pub lo: i16,
    pub hi: i16,
    pub span: u16,
}

impl Block {
    /// A lane's cell as a block of one.
    pub fn of([lo, hi]: [i16; 2]) -> Block {
        Block {
            lo,
            hi,
            span: (i32::from(hi) - i32::from(lo)).max(0) as u16,
        }
    }

    fn join(self, o: Block) -> Block {
        Block {
            lo: self.lo.min(o.lo),
            hi: self.hi.max(o.hi),
            span: self.span.max(o.span),
        }
    }
}

impl Index {
    /// A page's index from its lane, with its blocks.
    pub fn new(extent: Vec<[i16; 2]>, occupancy: Occupancy) -> Index {
        let mut blocks = vec![Block::default(); BLOCK_LANE];
        blocks_of(&extent, &mut blocks);
        Index {
            extent: extent.into(),
            blocks: blocks.into(),
            occupancy,
        }
    }

    /// The lane: each cell's lowest and highest, in `EXTENT`'s cell order.
    pub fn extent(&self) -> &[[i16; 2]] {
        &self.extent
    }

    /// What the `across` × `across` cells from (x, y) say, a block of the
    /// lane: `across` a power of two and (x, y) a multiple of it.
    pub fn over(&self, [x, y]: [u32; 2], across: u32) -> Block {
        match across.trailing_zeros() {
            0 => Block::of(self.extent[EXTENT.cell_index(x, y) as usize]),
            j => self.blocks[EXTENT.block_index(j, x, y) as usize],
        }
    }
}

/// `lane`'s blocks into `blocks`, each size from the one under it: the
/// smallest from the lane's cells, every other from the four a size down.
fn blocks_of(lane: &[[i16; 2]], blocks: &mut [Block]) {
    xpute_core::ensure!(lane.len() == EXTENT_LANE && blocks.len() == BLOCK_LANE, ENOTRECOVERABLE);
    let n = EXTENT.page_cells();
    for j in 1..=n.trailing_zeros() {
        let side = 1u32 << j;
        for by in (0..n).step_by(side as usize) {
            for bx in (0..n).step_by(side as usize) {
                let half = side / 2;
                let quarter = |dx: u32, dy: u32| match j {
                    1 => Block::of(lane[EXTENT.cell_index(bx + dx, by + dy) as usize]),
                    _ => blocks[EXTENT.block_index(j - 1, bx + dx, by + dy) as usize],
                };
                let b = quarter(0, 0).join(quarter(half, 0)).join(quarter(0, half)).join(quarter(half, half));
                blocks[EXTENT.block_index(j, bx, by) as usize] = b;
            }
        }
    }
}

// ---- Page ----

/// Static terrain payload carried by one L3 page.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub ground: Ground,
    pub decks: Decks,
    pub ele: std::borrow::Cow<'static, [i16]>,
    pub index: Index,
}

impl Terrain {
    /// A baked page of the finest level: its index is its own surface's,
    /// its decks' tops included.
    pub fn leaf(ele: Vec<i16>, ground: Ground, decks: Decks) -> Terrain {
        let extent = extent_of(&ele, &ground.cells, &decks);
        Terrain {
            ground,
            decks,
            ele: ele.into(),
            index: Index::new(extent, Occupancy::LEAF),
        }
    }
}

/// Decoded page payload with the baked terrain layer attached.
#[derive(Clone, Debug)]
pub struct Tile {
    pub terrain: Terrain,
}

// ---- DTO ----

// { ele, ground, index, decks } -> [ele, cells, extent, occupancy, decks]
//
// The cells go out as the client holds them, packed. The bake does that work
// once, offline, so a page arriving is a length check and a copy rather than
// a walk over a quarter of a million cells between the bytes landing and the
// ground being drawn from them.
pub fn encode_terrain(page: &Terrain) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&page.ele))
            .u16_array(Some(&page.ground.cells))
            .i16_array(Some(page.index.extent.as_flattened()))
            .u8(Some(page.index.occupancy.bits()));
        deck::write_branch(&page.decks, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The room a page is written into, as the client holds it: the record's own
/// lanes in `.fixed.terrain`, each a page's length, held for the page for as
/// long as it is resident.
pub struct PageRoom {
    pub cells: &'static mut [GroundCell],
    pub ele: &'static mut [i16],
    pub extent: &'static mut [[i16; 2]],
    pub blocks: &'static mut [Block],
    pub decks: deck::Room,
}

/// A packet's lanes, where they lie, and its occupancy.
struct Lanes<'a> {
    ele: &'a [i16],
    cells: &'a [GroundCell],
    extent: &'a [[i16; 2]],
    occupancy: Occupancy,
    decks: deck::Lanes<'a>,
}

/// The packet's lanes: refused unless each is a page's length, since a page
/// is nothing else.
fn read_page(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let cells = root.at(1)?.get_array::<GroundCell>()?;
    let extent = pairs(root.at(2)?.get_array::<i16>()?)?;
    let occupancy = occupancy_of(root.at(3)?.get_number()?)?;
    let decks = deck::read_branch(root.at_branch(4)?)?;
    if ele.len() != ELE_LANE || cells.len() != GROUND_LANE || extent.len() != EXTENT_LANE {
        return Err(MarshalError::new(Errno::EBADMSG));
    }
    Ok(Lanes { ele, cells, extent, occupancy, decks })
}

/// A fetched page straight into the room it will live in, read where it
/// lies: the index's blocks are made there before the page is handed back,
/// so nothing holds the page without them.
///
/// The wire carries what the client holds, so this is a length check and
/// three copies out of the packet — no page is allocated, the lane of the
/// index is walked once for its blocks and nothing else a cell at a time,
/// and nothing stands between a page arriving and the ground being drawn
/// from it. That is what packing at the bake buys.
pub fn decode_terrain_into(pkt: &[u8], room: PageRoom) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_page(pkt)?;
    if room.ele.len() != ELE_LANE || room.cells.len() != GROUND_LANE || room.extent.len() != EXTENT_LANE {
        return Err(MarshalError::new(Errno::EBADMSG));
    }
    room.ele.copy_from_slice(ele);
    room.cells.copy_from_slice(cells);
    room.extent.copy_from_slice(extent);
    blocks_of(room.extent, room.blocks);
    let PageRoom { cells, ele, extent, blocks, decks: deck_room } = room;
    Ok(Terrain {
        ground: Ground { cells: (&*cells).into() },
        decks: decks.into_room(deck_room),
        ele: (&*ele).into(),
        index: Index {
            extent: (&*extent).into(),
            blocks: (&*blocks).into(),
            occupancy,
        },
    })
}

// [ele, cells, extent, occupancy, decks] -> { ele, ground, index, decks }
//
// For whoever needs a page in hand rather than in a record: the bake's own
// reads, the tests. Copied out, because a decoded page outlives the packet
// it came from.
pub fn decode_terrain(pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_page(pkt)?;
    Ok(Terrain {
        ele: ele.to_vec().into(),
        ground: Ground { cells: cells.to_vec().into() },
        decks: decks.to_decks(),
        index: Index::new(extent.to_vec(), occupancy),
    })
}

/// The extent's pairs, which the wire carries flattened.
fn pairs(flat: &[i16]) -> Result<&[[i16; 2]], MarshalError> {
    match flat.as_chunks::<2>() {
        (pairs, []) => Ok(pairs),
        _ => Err(MarshalError::new(Errno::EBADMSG)),
    }
}

fn occupancy_of(number: f64) -> Result<Occupancy, MarshalError> {
    u8::try_from(number as i64).ok().and_then(Occupancy::of_bits).ok_or(MarshalError::new(Errno::EBADMSG))
}

#[cfg(test)]
#[path = "format.test.rs"]
mod test;
