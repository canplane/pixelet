// pxl-spec/bake/tile/terrain.rs

//! A baked terrain tile and its wire: `[heights, ground, occupancy, separation]`.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::{occupancy_of, Occupancy};
use crate::bake::constant::{level_at, BOUND_LEVELS, LEVEL_WORDS};
use crate::bake::malformed;
use crate::bake::separation::{self, Separation};
use crate::space::layer::lattice::TILE_CELLS;

/// A tile's lane lengths: a cell each; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const HEIGHT_LANE: usize = GROUND_LANE;
pub const LEVEL_LANE: usize = LEVEL_WORDS as usize;
/// Squares a side the ways' bound is held in (`Separation::bound`).
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_LANE: usize = (WAYS_ACROSS * WAYS_ACROSS) as usize;

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto look, which fits a byte (`block/cube.json`).
pub type GroundId = u8;

/// A tile's ground: one look a cell, the same shape from the bake through the
/// wire to the client.
#[derive(Clone, Debug, PartialEq)]
pub struct Ground {
    pub cells: Cow<'static, [GroundId]>,
}

impl Ground {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// A tile: one lane a fact the client cannot make. What it draws by rule —
/// a slab, a bore, a pier — is not here.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground,                 // a class a cell: the source rasterized, or its fold
    pub separation: Separation,         // ways off the ground: source lines, tops solved offline
    pub heights: Cow<'static, [i16]>,   // a cell's height in leaf cells: the surface's at a leaf, or its fold
    pub occupancy: Occupancy,           // which children are tiles: none is asked for that is not
    pub ways: Cow<'static, [[i16; 2]]>, // the ways' bound by square (`Separation::bound`): made from the lines wherever a tile is, never on the wire
    pub levels: Cow<'static, [i32]>,    // its cells' bound (`levels`), the ways' joined: made from them wherever a tile is, never on the wire
}

/// What a cell of a tile reaches: its height, and in its square any way's
/// top over it or a sunk way's own cell under it (`ways_of`).
pub fn cell_bound(heights: &[i16], ways: &[[i16; 2]], x: u32, y: u32) -> [i32; 2] {
    let h = i32::from(heights[(y * TILE_CELLS + x) as usize]);
    let s = TILE_CELLS / WAYS_ACROSS;
    let [lo, hi] = ways[((y / s) * WAYS_ACROSS + x / s) as usize];
    [h.min(i32::from(lo)), h.max(i32::from(hi))]
}

/// A tile's cells and ways, and its levels as far as they are made.
struct Made<'a> {
    heights: &'a [i16],
    ways: &'a [[i16; 2]],
    levels: &'a [i32],
}

impl pxl_portable::ground::Bound for Made<'_> {
    fn cell(&self, x: u32, y: u32) -> [i32; 2] {
        cell_bound(self.heights, self.ways, x, y)
    }
    fn level(&self, i: u32) -> i32 {
        self.levels[i as usize]
    }
}

/// The ways' bound off a tile's lines, by square, into `ways`.
pub fn ways_of(separation: &Separation, ways: &mut [[i16; 2]]) {
    separation.bound(f64::from(TILE_CELLS), WAYS_ACROSS, ways);
}

/// The levels of the bound over a tile's cells and ways (pxl-portable
/// `ground::join`), each over the one before, into `levels`.
pub fn levels(heights: &[i16], ways: &[[i16; 2]], levels: &mut [i32]) {
    xpute_kit::ensure!(heights.len() == HEIGHT_LANE && ways.len() == WAYS_LANE && levels.len() == LEVEL_LANE, ENOTRECOVERABLE);
    for j in 1..=BOUND_LEVELS {
        let (made, rest) = levels.split_at_mut(level_at(j) as usize);
        let side = TILE_CELLS >> j;
        for y in 0..side {
            for x in 0..side {
                rest[(y * side + x) as usize] = pxl_portable::ground::join(&Made { heights, ways, levels: made }, j, [x, y]);
            }
        }
    }
}

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_HEIGHTS: std::sync::LazyLock<Vec<i16>> = std::sync::LazyLock::new(|| vec![0; HEIGHT_LANE]);
static VOID_LEVELS: std::sync::LazyLock<Vec<i32>> = std::sync::LazyLock::new(|| vec![0; LEVEL_LANE]);
static VOID_WAYS: std::sync::LazyLock<Vec<[i16; 2]>> = std::sync::LazyLock::new(|| vec![[i16::MAX, i16::MIN]; WAYS_LANE]);
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> = std::sync::LazyLock::new(|| vec![crate::block::cube::sentinel::void; GROUND_LANE]);

impl TerrainTile {
    /// Where the source has nothing at all: void cells, which give no surface,
    /// so nothing is drawn while the ground is still known and stays asked
    /// for, and no child, so none is asked for under it.
    pub fn void() -> TerrainTile {
        TerrainTile {
            ground: Ground {
                cells: Cow::Borrowed(&VOID_CELLS[..]),
            },
            separation: Separation::default(),
            heights: Cow::Borrowed(&VOID_HEIGHTS[..]),
            occupancy: Occupancy::ZERO,
            ways: Cow::Borrowed(&VOID_WAYS[..]),
            levels: Cow::Borrowed(&VOID_LEVELS[..]),
        }
    }

    /// A leaf: its cells' looks and heights, and its separation.
    pub fn leaf(heights: Vec<i16>, ground: Ground, separation: Separation) -> TerrainTile {
        TerrainTile::of(heights, ground, separation, Occupancy::LEAF)
    }

    /// A tile of these lanes, its ways' bound and its levels made from them.
    pub fn of(heights: Vec<i16>, ground: Ground, separation: Separation, occupancy: Occupancy) -> TerrainTile {
        let mut ways = vec![[0; 2]; WAYS_LANE];
        ways_of(&separation, &mut ways);
        let mut made = vec![0; LEVEL_LANE];
        levels(&heights, &ways, &mut made);
        TerrainTile {
            ground,
            separation,
            heights: heights.into(),
            occupancy,
            ways: ways.into(),
            levels: made.into(),
        }
    }
}

// Cells go out packed, as the client holds them, so decoding is a length check and a copy.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&tile.heights)).u8_array(Some(&tile.ground.cells)).u8(Some(tile.occupancy.bits()));
        separation::write_branch(&tile.separation, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The record's lanes in `.fixed.terrain` a tile is decoded into.
pub struct TileRoom {
    pub ground: &'static mut [GroundId],
    pub heights: &'static mut [i16],
    pub ways: &'static mut [[i16; 2]],
    pub levels: &'static mut [i32],
    pub separation: separation::Room,
}

struct Lanes<'a> {
    heights: &'a [i16],
    ground: &'a [GroundId],
    occupancy: Occupancy,
    separation: separation::Lanes<'a>,
}

/// Refused unless every lane is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let heights = root.at(0)?.get_array::<i16>()?;
    let ground = root.at(1)?.get_array::<GroundId>()?;
    let occupancy = occupancy_of(root.at(2)?.get_number()?)?;
    let separation = separation::read_branch(root.at_branch(3)?)?;
    if heights.len() != HEIGHT_LANE || ground.len() != GROUND_LANE {
        return Err(malformed());
    }
    Ok(Lanes {
        heights,
        ground,
        occupancy,
        separation,
    })
}

/// Decode straight into the tile's resident room, its ways' bound and its
/// levels made there from its lines and heights.
pub fn decode_into(pkt: &[u8], room: TileRoom) -> Result<TerrainTile, MarshalError> {
    let Lanes {
        heights,
        ground,
        occupancy,
        separation,
    } = read_tile(pkt)?;
    if room.heights.len() != HEIGHT_LANE || room.ways.len() != WAYS_LANE || room.levels.len() != LEVEL_LANE || room.ground.len() != GROUND_LANE {
        return Err(malformed());
    }
    room.heights.copy_from_slice(heights);
    room.ground.copy_from_slice(ground);
    let TileRoom {
        ground,
        heights,
        ways,
        levels: made,
        separation: separation_room,
    } = room;
    let separation = separation.into_room(separation_room);
    ways_of(&separation, ways);
    levels(heights, ways, made);
    Ok(TerrainTile {
        ground: Ground { cells: (&*ground).into() },
        separation,
        heights: (&*heights).into(),
        occupancy,
        ways: (&*ways).into(),
        levels: (&*made).into(),
    })
}

// Copied out, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Lanes {
        heights,
        ground,
        occupancy,
        separation,
    } = read_tile(pkt)?;
    Ok(TerrainTile::of(heights.to_vec(), Ground { cells: ground.to_vec().into() }, separation.to_separation(), occupancy))
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
