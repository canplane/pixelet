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
//! baked wire = [ele, cells]
//!   cells = one u16 a cell, the four ground channels packed into it
//!           (bake/pack.rs), which is the shape the client holds too

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use super::pack::{channel_of, GroundCell};

// ---- DB ----

pub const BUILDING_ROW_KEYS: [&str; 6] = ["gers_id", "subtype", "class", "height", "num_floors", "geom_wkb"];
pub struct BuildingRow {
    pub gers_id: String,
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub height: Option<f64>,
    pub num_floors: Option<f64>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub const SEGMENT_ROW_KEYS: [&str; 5] = ["gers_id", "subtype", "class", "road", "geom_wkb"];
pub struct SegmentRow {
    pub gers_id: String,
    pub subtype: String,
    pub class: Option<String>,
    pub road: Option<String>, // raw JSON text (::text of the road JSONB column)
    pub geom_wkb: Box<[u8]>,  // WKB
}

pub const BASE_ROW_KEYS: [&str; 5] = ["gers_id", "source", "subtype", "class", "geom_wkb"];
pub struct BaseRow {
    pub gers_id: String,
    pub source: String, // "land_use" | "water" | "land" | "infrastructure"
    pub subtype: Option<String>,
    pub class: Option<String>,
    pub geom_wkb: Box<[u8]>, // WKB
}

pub const COAST_ROW_KEYS: [&str; 3] = ["id", "geom_wkb", "covers_page"];
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

// ---- Page ----

/// Static terrain payload carried by one L3 page.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub ground: Ground,
    pub ele: std::borrow::Cow<'static, [i16]>,
}

/// Decoded page payload with the baked terrain layer attached.
#[derive(Clone, Debug)]
pub struct Tile {
    pub terrain: Terrain,
}

// ---- DTO ----

// { ele, ground } -> [ele, cells]
//
// The cells go out as the client holds them, packed. The bake does that work
// once, offline, so a page arriving is a length check and a copy rather than
// a walk over a quarter of a million cells between the bytes landing and the
// ground being drawn from them.
pub fn encode_terrain(ele: &[i16], ground: &Ground) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(ele)).u16_array(Some(&ground.cells));
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The room a page is written into, as the client holds it: the record's own
/// lanes in `.fixed.terrain`.
pub struct PageRoom<'a> {
    pub cells: &'a mut [GroundCell],
    pub ele: &'a mut [i16],
}

/// A fetched page straight into the room it will live in.
///
/// The wire carries what the client holds, so this is a length check and two
/// copies out of the packet — no page is allocated, nothing is walked a cell
/// at a time, and nothing stands between a page arriving and the ground
/// being drawn from it. That is what packing at the bake buys.
pub fn decode_terrain_into(pkt: &[u8], room: PageRoom) -> Result<(), MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let cells = root.at(1)?.get_array::<GroundCell>()?;
    let wrong = || Err(MarshalError::new(Errno::EBADMSG));
    if ele.len() != room.ele.len() {
        return wrong();
    }
    if cells.len() != room.cells.len() {
        return wrong();
    }
    room.ele.copy_from_slice(ele);
    room.cells.copy_from_slice(cells);
    Ok(())
}

// [ele, cells] -> { ele, ground }
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
    })
}

#[cfg(test)]
#[path = "format.test.rs"]
mod test;
