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
//! raw wire   = [ele, buildings, segments, base, coasts]
//!   buildings = [[gers_id, subtype, class, height, num_floors, geom_wkb], ...]
//!   segments  = [[gers_id, subtype, class, road, geom_wkb], ...]
//!   base      = [[gers_id, source, subtype, class, geom_wkb], ...]  (source: land_use|water|land|infrastructure)
//!   coasts    = [[id, geom_wkb, covers_page], ...]
//!
//! baked wire = [ele, cells, extent, occupancy]
//!   cells     = one u16 a cell, the four ground channels packed into it
//!               (bake/pack.rs), which is the shape the client holds too
//!   extent    = the index lane's (lowest, highest) a cell, in `EXTENT`'s
//!               cell order, flattened to i16 pairs (`Index`)
//!   occupancy = one u8: which of the page and its four children are pages
//!               (`Occupancy`)

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use super::pack::{channel_of, GroundCell};
use super::surface::extent_of;

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
    pub road: Option<String>, // raw JSON text (::text of the road JSONB column)
    pub geom_wkb: Box<[u8]>,  // WKB
}

pub struct BaseRow {
    pub gers_id: String,
    pub source: String, // "land_use" | "water" | "land" | "infrastructure"
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>, // WKB
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
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    pub extent: std::borrow::Cow<'static, [[i16; 2]]>,
    pub occupancy: Occupancy,
}

// ---- Page ----

/// Static terrain payload carried by one L3 page.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub ground: Ground,
    pub ele: std::borrow::Cow<'static, [i16]>,
    pub index: Index,
}

impl Terrain {
    /// A baked page of the finest level: its index is its own surface's.
    pub fn leaf(ele: Vec<i16>, ground: Ground) -> Terrain {
        let extent = extent_of(&ele, &ground.cells);
        Terrain {
            ground,
            ele: ele.into(),
            index: Index {
                extent: extent.into(),
                occupancy: Occupancy::LEAF,
            },
        }
    }
}

/// Decoded page payload with the baked terrain layer attached.
#[derive(Clone, Debug)]
pub struct Tile {
    pub terrain: Terrain,
}

// ---- DTO ----

// { ele, ground, index } -> [ele, cells, extent, occupancy]
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
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The room a page is written into, as the client holds it: the record's own
/// lanes in `.fixed.terrain`.
pub struct PageRoom<'a> {
    pub cells: &'a mut [GroundCell],
    pub ele: &'a mut [i16],
    pub extent: &'a mut [[i16; 2]],
}

/// A fetched page straight into the room it will live in, and its
/// occupancy, a byte the page carries by value rather than in a lane.
///
/// The wire carries what the client holds, so this is a length check and
/// three copies out of the packet — no page is allocated, nothing is walked a
/// cell at a time, and nothing stands between a page arriving and the ground
/// being drawn from it. That is what packing at the bake buys.
pub fn decode_terrain_into(pkt: &[u8], room: PageRoom) -> Result<Occupancy, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let cells = root.at(1)?.get_array::<GroundCell>()?;
    let extent = pairs(root.at(2)?.get_array::<i16>()?)?;
    let occupancy = occupancy_of(root.at(3)?.get_number()?)?;
    if ele.len() != room.ele.len() || cells.len() != room.cells.len() || extent.len() != room.extent.len() {
        return Err(MarshalError::new(Errno::EBADMSG));
    }
    room.ele.copy_from_slice(ele);
    room.cells.copy_from_slice(cells);
    room.extent.copy_from_slice(extent);
    Ok(occupancy)
}

// [ele, cells, extent, occupancy] -> { ele, ground, index }
//
// For whoever needs a page in hand rather than in a record: the shared ocean
// tile, the bake's own reads, the tests. Copied out, because a decoded page
// outlives the packet it came from.
pub fn decode_terrain(pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    Ok(Terrain {
        ele: root.at(0)?.get_array::<i16>()?.to_vec().into(),
        ground: Ground {
            cells: root.at(1)?.get_array::<GroundCell>()?.to_vec().into(),
        },
        index: Index {
            extent: pairs(root.at(2)?.get_array::<i16>()?)?.to_vec().into(),
            occupancy: occupancy_of(root.at(3)?.get_number()?)?,
        },
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
