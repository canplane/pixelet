// pxl-spec/bake/format.rs

//! A baked terrain tile and its wire: `[ele, cells, extent, occupancy, decks]`.

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::constant::ELE_PADDED_SPAN;
use super::deck::{self, Decks};
use super::ground::SHIFT_RANGE;
use super::malformed;
use super::surface::extent_of;
use crate::space::layer::lattice::{EXTENT, TILE_CELLS};

/// A tile's lane lengths; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
pub const EXTENT_LANE: usize = (EXTENT.tile_cells() * EXTENT.tile_cells()) as usize;
pub const COARSE_LANE: usize = EXTENT.coarse_cells() as usize;

/// A ground cell: one look of the Carto palette, a byte (`cell/cube_type.json`).
pub type GroundCell = u8;

/// A tile's ground: one look a cell, the same shape from the bake through the
/// wire to the client.
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundCell]>,
}

impl Ground {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Which of a tile and its four children are tiles of their own, the rest
/// said whole by the tile (terrain's zero tile, POI's uncrowded ground):
/// children in the low four bits in `slot_of`'s order, the tile itself above
/// them. A client never asks for a child its parent marks absent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Occupancy(u8);

impl Occupancy {
    const TILE: u8 = 1 << 4;

    pub const ZERO: Occupancy = Occupancy(0);

    pub const LEAF: Occupancy = Occupancy(Occupancy::TILE);

    pub fn of_children(children: [Occupancy; 4]) -> Occupancy {
        Occupancy::of_slots(children.map(Occupancy::is_tile))
    }

    /// A tile whose children in `slot_of`'s order are tiles where `slots` says.
    pub fn of_slots(slots: [bool; 4]) -> Occupancy {
        let bits = (0..4).filter(|&q| slots[q]).fold(0, |bits, q| bits | 1 << q);
        Occupancy(if bits == 0 { 0 } else { Occupancy::TILE | bits })
    }

    pub fn is_tile(self) -> bool {
        self.0 & Occupancy::TILE != 0
    }

    pub fn has_child(self, slot: usize) -> bool {
        self.0 & (1 << slot) != 0
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// None for a byte no tile can have: a bit past the five, or a child without its parent.
    pub fn of_bits(bits: u8) -> Option<Occupancy> {
        (bits == 0 || (bits & Occupancy::TILE != 0 && bits < Occupancy::TILE << 1)).then_some(Occupancy(bits))
    }
}

/// What a ray and a fetch need before the tiles under one are held. At every
/// zoom, the leaf too, so a tile has one contract.
#[derive(Clone, Debug, PartialEq)]
pub struct Index {
    extent: Cow<'static, [[i16; 2]]>, // lo, hi for each `EXTENT` cell, over this zoom and all finer: a ray above hi misses them
    coarse: Cow<'static, [[i16; 2]]>, // `extent` joined over each coarser cell: rebuilt from it, never on the wire
    pub occupancy: Occupancy,         // which children are tiles: none is asked for that is not
}

/// A partial version's list, beside its tiles: the full version it overlays
/// and the tiles it writes or removes over it, by name. A version without
/// one is full.
pub const SNAPSHOT_KEY: &str = "_snapshot";

/// How far past the lane's facts the client draws a cell, down and up: the
/// ground's shift and a bore's floor. Every read of the lane widens by it.
pub const DRAWN_PAST: [i16; 2] = {
    let [lo, hi] = SHIFT_RANGE;
    // Ground shifted down, or the way's own cell under a bore's or a cutting's surface.
    [if lo < -1 { lo } else { -1 }, hi]
};

pub fn drawn([lo, hi]: [i16; 2]) -> [i16; 2] {
    [lo.saturating_add(DRAWN_PAST[0]), hi.saturating_add(DRAWN_PAST[1])]
}

/// Two extents as one: the lower of the lows, the higher of the highs.
pub fn join([lo, hi]: [i16; 2], [o_lo, o_hi]: [i16; 2]) -> [i16; 2] {
    [lo.min(o_lo), hi.max(o_hi)]
}

impl Index {
    pub fn new(extent: Vec<[i16; 2]>, occupancy: Occupancy) -> Index {
        let mut coarse = vec![[0; 2]; COARSE_LANE];
        coarse_of(&extent, &mut coarse);
        Index {
            extent: extent.into(),
            coarse: coarse.into(),
            occupancy,
        }
    }

    pub fn extent(&self) -> &[[i16; 2]] {
        &self.extent
    }

    /// What is drawn over the cell of `across` × `across` index cells at
    /// (x, y), widened by `drawn`; `across` a power of two and (x, y) a multiple of it.
    pub fn over(&self, [x, y]: [u32; 2], across: u32) -> [i16; 2] {
        drawn(match across.trailing_zeros() {
            0 => self.extent[EXTENT.cell_index(x, y) as usize],
            j => self.coarse[EXTENT.coarse_index(j, x, y) as usize],
        })
    }
}

/// Each coarser cell from the four under it.
fn coarse_of(lane: &[[i16; 2]], coarse: &mut [[i16; 2]]) {
    xpute_core::ensure!(lane.len() == EXTENT_LANE && coarse.len() == COARSE_LANE, ENOTRECOVERABLE);
    let n = EXTENT.tile_cells();
    for j in 1..=n.trailing_zeros() {
        let side = 1u32 << j;
        for by in (0..n).step_by(side as usize) {
            for bx in (0..n).step_by(side as usize) {
                let half = side / 2;
                let quarter = |dx: u32, dy: u32| match j {
                    1 => lane[EXTENT.cell_index(bx + dx, by + dy) as usize],
                    _ => coarse[EXTENT.coarse_index(j - 1, bx + dx, by + dy) as usize],
                };
                let b = join(join(quarter(0, 0), quarter(half, 0)), join(quarter(0, half), quarter(half, half)));
                coarse[EXTENT.coarse_index(j, bx, by) as usize] = b;
            }
        }
    }
}

/// A tile: one lane a fact the client cannot make. What it draws by rule —
/// a slab, a bore, a pier — is not here.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    pub ground: Ground,           // a class a cell: the source rasterized
    pub decks: Decks,             // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>, // padded height samples, in leaf cells: from the raster
    pub index: Index,             // bounds over the tile and all under it: no one tile holds them
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_ELE: std::sync::LazyLock<Vec<i16>> = std::sync::LazyLock::new(|| vec![0; ELE_LANE]);
static VOID_CELLS: std::sync::LazyLock<Vec<GroundCell>> = std::sync::LazyLock::new(|| vec![crate::cell::cube_types::sentinel::void; GROUND_LANE]);

impl Terrain {
    /// Where the source has nothing at all: void cells, which give no surface,
    /// so nothing is drawn while the ground is still known and stays asked
    /// for, and no child, so none is asked for under it.
    pub fn void() -> Terrain {
        Terrain {
            ground: Ground {
                cells: Cow::Borrowed(&VOID_CELLS[..]),
            },
            decks: Decks::default(),
            ele: Cow::Borrowed(&VOID_ELE[..]),
            index: Index::new(vec![[0, 0]; EXTENT_LANE], Occupancy::ZERO),
        }
    }

    /// A leaf, its index taken from its own surface and decks.
    pub fn leaf(ele: Vec<i16>, ground: Ground, decks: Decks) -> Terrain {
        let extent = extent_of(&ele, &decks);
        Terrain {
            ground,
            decks,
            ele: ele.into(),
            index: Index::new(extent, Occupancy::LEAF),
        }
    }
}

// Cells go out packed, as the client holds them, so decoding is a length check and a copy.
pub fn encode_terrain(tile: &Terrain) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&tile.ele))
            .u8_array(Some(&tile.ground.cells))
            .i16_array(Some(tile.index.extent.as_flattened()))
            .u8(Some(tile.index.occupancy.bits()));
        deck::write_branch(&tile.decks, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The record's lanes in `.fixed.terrain` a tile is decoded into.
pub struct TileRoom {
    pub cells: &'static mut [GroundCell],
    pub ele: &'static mut [i16],
    pub extent: &'static mut [[i16; 2]],
    pub coarse: &'static mut [[i16; 2]],
    pub decks: deck::Room,
}

struct Lanes<'a> {
    ele: &'a [i16],
    cells: &'a [GroundCell],
    extent: &'a [[i16; 2]],
    occupancy: Occupancy,
    decks: deck::Lanes<'a>,
}

/// Refused unless every lane is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let cells = root.at(1)?.get_array::<GroundCell>()?;
    let extent = pairs(root.at(2)?.get_array::<i16>()?)?;
    let occupancy = occupancy_of(root.at(3)?.get_number()?)?;
    let decks = deck::read_branch(root.at_branch(4)?)?;
    if ele.len() != ELE_LANE || cells.len() != GROUND_LANE || extent.len() != EXTENT_LANE {
        return Err(malformed());
    }
    Ok(Lanes { ele, cells, extent, occupancy, decks })
}

/// Decode straight into the tile's resident room; the index's coarse are
/// built there before the tile is returned.
pub fn decode_terrain_into(pkt: &[u8], room: TileRoom) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_tile(pkt)?;
    if room.ele.len() != ELE_LANE || room.cells.len() != GROUND_LANE || room.extent.len() != EXTENT_LANE {
        return Err(malformed());
    }
    room.ele.copy_from_slice(ele);
    room.cells.copy_from_slice(cells);
    room.extent.copy_from_slice(extent);
    coarse_of(room.extent, room.coarse);
    let TileRoom {
        cells,
        ele,
        extent,
        coarse,
        decks: deck_room,
    } = room;
    Ok(Terrain {
        ground: Ground { cells: (&*cells).into() },
        decks: decks.into_room(deck_room),
        ele: (&*ele).into(),
        index: Index {
            extent: (&*extent).into(),
            coarse: (&*coarse).into(),
            occupancy,
        },
    })
}

// Copied out, for a tile that outlives its packet.
pub fn decode_terrain(pkt: &[u8]) -> Result<Terrain, MarshalError> {
    let Lanes { ele, cells, extent, occupancy, decks } = read_tile(pkt)?;
    Ok(Terrain {
        ele: ele.to_vec().into(),
        ground: Ground { cells: cells.to_vec().into() },
        decks: decks.to_decks(),
        index: Index::new(extent.to_vec(), occupancy),
    })
}

fn pairs(flat: &[i16]) -> Result<&[[i16; 2]], MarshalError> {
    match flat.as_chunks::<2>() {
        (pairs, []) => Ok(pairs),
        _ => Err(malformed()),
    }
}

/// A whole number a byte holds, never a cast of one that is not.
pub fn occupancy_of(number: f64) -> Result<Occupancy, MarshalError> {
    let whole = number.fract() == 0.0 && (0.0..=255.0).contains(&number);
    whole.then_some(number as u8).and_then(Occupancy::of_bits).ok_or_else(malformed)
}

#[cfg(test)]
#[path = "format.test.rs"]
mod test;
