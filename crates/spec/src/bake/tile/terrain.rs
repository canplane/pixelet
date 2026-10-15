// pxl-spec/bake/tile/terrain.rs

//! A baked terrain tile and its wire: `[samples, ground, occupancy, separation]`.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::{occupancy_of, Occupancy};
use crate::bake::constant::{level_at, BOUND_LEVELS, ELE_PADDED_SPAN, LEVEL_WORDS};
use crate::bake::malformed;
use crate::bake::separation::{self, Separation};
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};

/// A tile's lane lengths: a cell each, four looks a cell, and its padded
/// samples; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const LOOK_LANE: usize = 4 * GROUND_LANE;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
pub const HEIGHT_LANE: usize = GROUND_LANE;
pub const LEVEL_LANE: usize = LEVEL_WORDS as usize;
/// Squares a side the ways' bound is held in (`Separation::bound`), each
/// `1 << WAYS_SHIFT` cells a side, then its levels up to the tile whole.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
const WAYS_LEVELS: u32 = WAYS_ACROSS.trailing_zeros();
pub const WAYS_LANE: usize = ways_at(WAYS_LEVELS + 1) as usize;

/// The most entries a window's square holds of the ways: one a segment and
/// a square of the bound (`WAYS_SHIFT`) its footprint reaches. The densest
/// square of the format-10 set, z15 010332102333322 (2026-10-15); a tile
/// past it fails its bake (`ways_fit`).
pub const WAYS_SQUARE_SEGMENTS: usize = 1068;

/// Where level `k` of the ways' bound begins, the squares level 0, in entries.
const fn ways_at(k: u32) -> u32 {
    let n = WAYS_ACROSS * WAYS_ACROSS;
    4 * (n - (n >> (2 * k))) / 3
}

/// What a ground cell holds: the id of a cube of the ground's palette, a
/// Carto look, which fits a byte (`block/cube.json`).
pub type GroundId = u8;

/// A tile's ground: in the bake the four looks a cell folds, `dx + 2·dy`, as
/// a GPU holds them, a leaf's its own four; on the wire one a cell where
/// every cell's four are one; on the CPU each cell's own look (`decode_into`).
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
/// a slab, a bore, a pier — is not here. The bake holds what goes on the
/// wire; a client what it looks up, its cells drawn by a GPU off the samples
/// and read back.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground,                 // a class a cell: the source rasterized, or its fold
    pub separation: Separation,         // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>,       // the bake's: padded samples, a leaf's source or the fold's; a client hands them to a GPU off the packet
    pub heights: Cow<'static, [i16]>,   // a client's: each cell's height as the GPU drew it from `ele`, read back
    pub occupancy: Occupancy,           // which children are tiles: none is asked for that is not
    pub ways: Cow<'static, [[i16; 2]]>, // a client's: the ways' bound by square and its levels, made from the lines
    pub levels: Cow<'static, [i32]>,    // a client's: the bound over its cells the GPU made, read back
}

/// What a cell of a tile reaches: its height, and in its square any way's
/// top over it or a sunk way's own cell under it (`ways_of`).
pub fn cell_bound(heights: &[i16], ways: &[[i16; 2]], x: u32, y: u32) -> [i32; 2] {
    let h = i32::from(heights[(y * TILE_CELLS + x) as usize]);
    let [lo, hi] = ways[((y >> WAYS_SHIFT) * WAYS_ACROSS + (x >> WAYS_SHIFT)) as usize];
    [h.min(i32::from(lo)), h.max(i32::from(hi))]
}

/// Entry `(x, y)` of level `j` of a tile's bound as the CPU reads it: the
/// cells' (`levels`, the GPU's) joined with the ways' over the same cells,
/// their square's or a level of squares.
pub fn level_bound(levels: &[i32], ways: &[[i16; 2]], j: u32, [x, y]: [u32; 2]) -> i32 {
    let [lo, hi] = pxl_portable::ground::bound_of(levels[(level_at(j) + y * (TILE_CELLS >> j) + x) as usize]);
    let [wlo, whi] = if j <= WAYS_SHIFT {
        ways[(((y << j) >> WAYS_SHIFT) * WAYS_ACROSS + ((x << j) >> WAYS_SHIFT)) as usize]
    } else {
        let k = j - WAYS_SHIFT;
        ways[(ways_at(k) + y * (WAYS_ACROSS >> k) + x) as usize]
    };
    pxl_portable::ground::bound_word([lo.min(i32::from(wlo)), hi.max(i32::from(whi))])
}

/// The ways' bound off a tile's lines, by square, then its levels, each
/// entry the join of the four under it, into `ways`.
pub fn ways_of(separation: &Separation, ways: &mut [[i16; 2]]) {
    xpute_kit::ensure!(ways.len() == WAYS_LANE, ENOTRECOVERABLE);
    separation.bound(f64::from(TILE_CELLS), WAYS_ACROSS, &mut ways[..(WAYS_ACROSS * WAYS_ACROSS) as usize]);
    for k in 1..=WAYS_LEVELS {
        let (under, side) = (ways_at(k - 1), WAYS_ACROSS >> k);
        for y in 0..side {
            for x in 0..side {
                let four = [0, 1, 2, 3].map(|q| ways[(under + (2 * y + q / 2) * 2 * side + 2 * x + q % 2) as usize]);
                ways[(ways_at(k) + y * side + x) as usize] = four.into_iter().fold([i16::MAX, i16::MIN], |[lo, hi], [l, h]| [lo.min(l), hi.max(h)]);
            }
        }
    }
}

/// Whether every square of `square` cells a side a tile is cut into holds
/// its ways' entries within `WAYS_SQUARE_SEGMENTS`; the first that does not,
/// its place in squares and its count, where one does not.
pub fn ways_fit(separation: &Separation, square: u32) -> Result<(), ([u32; 2], usize)> {
    let mut entries = [0usize; (WAYS_ACROSS * WAYS_ACROSS) as usize];
    separation.reaches(f64::from(TILE_CELLS), WAYS_ACROSS, |k, _, _| entries[k] += 1);
    let (bins, squares) = (square >> WAYS_SHIFT, TILE_CELLS / square);
    for qy in 0..squares {
        for qx in 0..squares {
            let n: usize = (0..bins * bins).map(|b| entries[((qy * bins + b / bins) * WAYS_ACROSS + qx * bins + b % bins) as usize]).sum();
            if n > WAYS_SQUARE_SEGMENTS {
                return Err(([qx, qy], n));
            }
        }
    }
    Ok(())
}

/// A tile's cells, the levels as far as they are made over them.
struct Made<'a> {
    heights: &'a [i16],
    levels: &'a [i32],
}

impl pxl_portable::ground::Bound for Made<'_> {
    fn cell(&self, x: u32, y: u32) -> [i32; 2] {
        let h = i32::from(self.heights[(y * TILE_CELLS + x) as usize]);
        [h, h]
    }
    fn level(&self, j: u32, [x, y]: [u32; 2]) -> i32 {
        self.levels[(level_at(j) + y * (TILE_CELLS >> j) + x) as usize]
    }
}

/// The levels of the bound over a tile's cells (pxl-portable
/// `ground::join`), each over the one before, into `levels`: what a GPU
/// makes of the cells it drew (`bound_cs`), for a client with none to read
/// them back from, the kernel's tests.
pub fn levels(heights: &[i16], levels: &mut [i32]) {
    xpute_kit::ensure!(heights.len() == HEIGHT_LANE && levels.len() == LEVEL_LANE, ENOTRECOVERABLE);
    for j in 1..=BOUND_LEVELS {
        let (made, rest) = levels.split_at_mut(level_at(j) as usize);
        let side = TILE_CELLS >> j;
        for y in 0..side {
            for x in 0..side {
                rest[(y * side + x) as usize] = pxl_portable::ground::join(&Made { heights, levels: made }, j, [x, y]);
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
    /// Where the source has nothing at all, as a client holds it: void
    /// cells, which give no surface, so nothing is drawn while the ground is
    /// still known and stays asked for, and no child, so none is asked for
    /// under it.
    pub fn void() -> TerrainTile {
        TerrainTile {
            ground: Ground {
                cells: Cow::Borrowed(&VOID_CELLS[..]),
            },
            separation: Separation::default(),
            ele: Cow::Borrowed(&[]),
            heights: Cow::Borrowed(&VOID_HEIGHTS[..]),
            occupancy: Occupancy::ZERO,
            ways: Cow::Borrowed(&VOID_WAYS[..]),
            levels: Cow::Borrowed(&VOID_LEVELS[..]),
        }
    }

    /// A leaf as the bake makes it: its padded samples, a look a cell, each
    /// its own four, and its separation.
    pub fn leaf(ele: Vec<i16>, ground: Ground, separation: Separation) -> TerrainTile {
        let ground = Ground { cells: expand(&ground.cells).into() };
        TerrainTile::of(ele, ground, separation, Occupancy::LEAF)
    }

    /// A tile as the bake holds it: these lanes, nothing a client looks up.
    pub fn of(ele: Vec<i16>, ground: Ground, separation: Separation, occupancy: Occupancy) -> TerrainTile {
        TerrainTile {
            ground,
            separation,
            ele: ele.into(),
            heights: Cow::Borrowed(&[]),
            occupancy,
            ways: Cow::Borrowed(&[]),
            levels: Cow::Borrowed(&[]),
        }
    }

    /// A tile as a client holds it once a GPU has drawn its cells: each
    /// cell's own look and height, its lines, and the ways' bound and the
    /// cells' levels made from them; for a client with no GPU to read them
    /// back from, the kernel's tests.
    pub fn drawn(heights: Vec<i16>, ground: Ground, separation: Separation, occupancy: Occupancy) -> TerrainTile {
        let mut ways = vec![[0; 2]; WAYS_LANE];
        ways_of(&separation, &mut ways);
        let mut made = vec![0; LEVEL_LANE];
        levels(&heights, &mut made);
        TerrainTile {
            ground,
            separation,
            ele: Cow::Borrowed(&[]),
            heights: heights.into(),
            occupancy,
            ways: ways.into(),
            levels: made.into(),
        }
    }
}

/// One look a cell, each its own four.
fn expand(one: &[GroundId]) -> Vec<GroundId> {
    one.iter().flat_map(|&l| [l; 4]).collect()
}

// A tile whose every cell's four looks are one, a leaf's, goes out a look a
// cell, expanded on decode.
pub fn encode(tile: &TerrainTile) -> Result<Vec<u8>, MarshalError> {
    let four = &tile.ground.cells;
    let one: Option<Vec<GroundId>> = four.chunks(4).all(|q| q.iter().all(|&l| l == q[0])).then(|| four.chunks(4).map(|q| q[0]).collect());
    let looks: &[GroundId] = one.as_deref().unwrap_or(four);
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.i16_array(Some(&tile.ele)).u8_array(Some(looks)).u8(Some(tile.occupancy.bits()));
        separation::write_branch(&tile.separation, b);
    }));
    encoder().encode(&t, TreeEncoderOptions::default())
}

/// The record's lanes in `.fixed.terrain` a tile is decoded into: a cell's
/// own look alone, the four it folds the GPU's (`handed`); its heights and
/// levels, which come back from the GPU there.
pub struct TileRoom {
    pub ground: &'static mut [GroundId],
    pub heights: &'static mut [i16],
    pub ways: &'static mut [[i16; 2]],
    pub levels: &'static mut [i32],
    pub separation: separation::Room,
}

struct Lanes<'a> {
    ele: &'a [i16],
    ground: &'a [GroundId],
    occupancy: Occupancy,
    separation: separation::Lanes<'a>,
}

/// Refused unless every lane is a tile's length.
fn read_tile(pkt: &[u8]) -> Result<Lanes<'_>, MarshalError> {
    let root = TreeReader::new(pkt)?.read_branch()?;
    let ele = root.at(0)?.get_array::<i16>()?;
    let ground = root.at(1)?.get_array::<GroundId>()?;
    let occupancy = occupancy_of(root.at(2)?.get_number()?)?;
    let separation = separation::read_branch(root.at_branch(3)?)?;
    if ele.len() != ELE_LANE || (ground.len() != GROUND_LANE && ground.len() != LOOK_LANE) {
        return Err(malformed());
    }
    Ok(Lanes { ele, ground, occupancy, separation })
}

/// What a GPU is handed off a packet: its looks, four a cell, a lane of
/// one look a cell expanded into `room`, each its own four; and its padded
/// samples.
pub fn handed<'a>(pkt: &'a [u8], room: &'a mut [GroundId]) -> Result<(&'a [GroundId], &'a [i16]), MarshalError> {
    let Lanes { ele, ground, .. } = read_tile(pkt)?;
    if ground.len() == LOOK_LANE {
        return Ok((ground, ele));
    }
    for (q, &l) in room[..LOOK_LANE].chunks_mut(4).zip(ground) {
        q.fill(l);
    }
    Ok((&room[..LOOK_LANE], ele))
}

/// Decode the tile at `zoom` straight into its resident room: each cell's
/// own look (pxl-portable `ground::own`), its lines, and the ways' bound
/// made from them. Its heights and levels are the room's as they come back
/// from the GPU.
pub fn decode_into(pkt: &[u8], zoom: Zoom, room: TileRoom) -> Result<TerrainTile, MarshalError> {
    let Lanes { ground, occupancy, separation, .. } = read_tile(pkt)?;
    if room.heights.len() != HEIGHT_LANE || room.ways.len() != WAYS_LANE || room.levels.len() != LEVEL_LANE || room.ground.len() != GROUND_LANE {
        return Err(malformed());
    }
    if ground.len() == GROUND_LANE {
        room.ground.copy_from_slice(ground);
    } else {
        let cell = GROUND.cell_zoom(zoom);
        for (l, &q) in room.ground.iter_mut().zip(ground.as_chunks::<4>().0) {
            *l = pxl_portable::ground::own(u32::from_le_bytes(q), cell) as GroundId;
        }
    }
    let TileRoom {
        ground,
        heights,
        ways,
        levels,
        separation: separation_room,
    } = room;
    let separation = separation.into_room(separation_room);
    ways_of(&separation, ways);
    Ok(TerrainTile {
        ground: Ground { cells: (&*ground).into() },
        separation,
        ele: Cow::Borrowed(&[]),
        heights: (&*heights).into(),
        occupancy,
        ways: (&*ways).into(),
        levels: (&*levels).into(),
    })
}

// Copied out as the bake holds it, for a tile that outlives its packet.
pub fn decode(pkt: &[u8]) -> Result<TerrainTile, MarshalError> {
    let Lanes { ele, ground, occupancy, separation } = read_tile(pkt)?;
    let cells = if ground.len() == LOOK_LANE { ground.to_vec() } else { expand(ground) };
    Ok(TerrainTile::of(ele.to_vec(), Ground { cells: cells.into() }, separation.to_separation(), occupancy))
}

#[cfg(test)]
#[path = "terrain.test.rs"]
mod test;
