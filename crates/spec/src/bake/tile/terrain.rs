// pxl-spec/bake/tile/terrain.rs

//! A baked terrain tile and its wire: `[samples, ground, occupancy, separation]`.

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

use std::borrow::Cow;

use super::{occupancy_of, Occupancy};
use crate::bake::constant::ELE_PADDED_SPAN;
use crate::bake::malformed;
use crate::bake::separation::{self, Separation};
use crate::space::layer::lattice::TILE_CELLS;

/// A tile's lane lengths: a cell each, four looks a cell, and its padded
/// samples; a tile is these and no other size.
pub const GROUND_LANE: usize = (TILE_CELLS * TILE_CELLS) as usize;
pub const LOOK_LANE: usize = 4 * GROUND_LANE;
pub const ELE_LANE: usize = (ELE_PADDED_SPAN * ELE_PADDED_SPAN) as usize;
/// Squares a side the ways' bound is held in (`Separation::bound`), each
/// `1 << WAYS_SHIFT` cells a side, then its levels up to the tile whole
/// (pxl-portable `window`, which cuts a square's share).
pub use pxl_portable::window::{WAYS_ACROSS, WAYS_SHIFT};
const WAYS_TILE_LEVELS: u32 = WAYS_ACROSS.trailing_zeros();
pub const WAYS_LANE: usize = ways_at(WAYS_TILE_LEVELS + 1) as usize;

/// The most entries a window's square holds of the ways: one a segment and
/// a square of the bound (`WAYS_SHIFT`) its footprint reaches. The densest
/// square of the format-10 set, z15 010332102333322 (2026-10-15); a tile
/// past it fails its bake (`ways_fit`).
pub const WAYS_SQUARE_SEGMENTS: usize = 1068;

/// Where level `k` of the ways' bound begins, the squares level 0, in entries.
const fn ways_at(k: u32) -> u32 {
    pxl_portable::window::ways_at(WAYS_ACROSS, k)
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
/// wire; a client what it reads of a tile as a whole, its lines and the
/// ways' bound made from them. Its cells a client holds in its windows,
/// drawn by a GPU off the samples (pxl-portable `window`).
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTile {
    pub ground: Ground,                 // a class a cell: the source rasterized, or its fold; a client's void tile's alone
    pub separation: Separation,         // ways off the ground: source lines, tops solved offline
    pub ele: Cow<'static, [i16]>,       // the bake's: padded samples, a leaf's source or the fold's; a client hands them to a GPU off the packet
    pub occupancy: Occupancy,           // which children are tiles: none is asked for that is not
    pub ways: Cow<'static, [[i16; 2]]>, // a client's: the ways' bound by square and its levels, made from the lines
}

/// The ways' bound off a tile's lines, by square, then its levels, each
/// entry the join of the four under it, into `ways`.
pub fn ways_of(separation: &Separation, ways: &mut [[i16; 2]]) {
    xpute_kit::ensure!(ways.len() == WAYS_LANE, ENOTRECOVERABLE);
    separation.bound(f64::from(TILE_CELLS), WAYS_ACROSS, &mut ways[..(WAYS_ACROSS * WAYS_ACROSS) as usize]);
    for k in 1..=WAYS_TILE_LEVELS {
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

// Made on first use: a constant array of zeros would ship in the module's data.
static VOID_WAYS: std::sync::LazyLock<Vec<[i16; 2]>> = std::sync::LazyLock::new(|| vec![[i16::MAX, i16::MIN]; WAYS_LANE]);
static VOID_CELLS: std::sync::LazyLock<Vec<GroundId>> = std::sync::LazyLock::new(|| vec![crate::block::cube::sentinel::void; GROUND_LANE]);

impl TerrainTile {
    /// Where the source has nothing at all, as a client holds it: void
    /// cells at sea level, which give no surface, so nothing is drawn while
    /// the ground is still known and stays asked for, and no child, so none
    /// is asked for under it. Its windows' squares are filled from it as a
    /// packet's are (`ground`, and samples all zero).
    pub fn void() -> TerrainTile {
        TerrainTile {
            ground: Ground {
                cells: Cow::Borrowed(&VOID_CELLS[..]),
            },
            separation: Separation::default(),
            ele: Cow::Borrowed(&[]),
            occupancy: Occupancy::ZERO,
            ways: Cow::Borrowed(&VOID_WAYS[..]),
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
            occupancy,
            ways: Cow::Borrowed(&[]),
        }
    }

    /// A tile as a client holds it: its lines and the ways' bound made from
    /// them, its cells none of its own (a client's windows hold them).
    pub fn held(separation: Separation, occupancy: Occupancy) -> TerrainTile {
        let mut ways = vec![[0; 2]; WAYS_LANE];
        ways_of(&separation, &mut ways);
        TerrainTile {
            ground: Ground { cells: Cow::Borrowed(&[]) },
            separation,
            ele: Cow::Borrowed(&[]),
            occupancy,
            ways: ways.into(),
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

/// The record's lanes in `.fixed.terrain` a tile is decoded into: its lines
/// and the ways' bound; its cells go to its windows (`handed`).
pub struct TileRoom {
    pub ways: &'static mut [[i16; 2]],
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

/// Decode a tile straight into its resident room: its lines, and the ways'
/// bound made from them.
pub fn decode_into(pkt: &[u8], room: TileRoom) -> Result<TerrainTile, MarshalError> {
    let Lanes { occupancy, separation, .. } = read_tile(pkt)?;
    if room.ways.len() != WAYS_LANE {
        return Err(malformed());
    }
    let TileRoom { ways, separation: separation_room } = room;
    let separation = separation.into_room(separation_room);
    ways_of(&separation, ways);
    Ok(TerrainTile {
        ground: Ground { cells: Cow::Borrowed(&[]) },
        separation,
        ele: Cow::Borrowed(&[]),
        occupancy,
        ways: (&*ways).into(),
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
