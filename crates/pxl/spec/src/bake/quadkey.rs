// pxl-spec/bake/quadkey.rs

//! A baked tile's object-store name: its place in its zoom's tile grid as a
//! quadkey, coarsest digit first. A coarser tile's name prefixes a finer one's
//! over the same ground, and every well-formed name is a tile.

use core::fmt;

use crate::space::pvf3::ivec::{self, IVec};
use crate::space::pyramid::{around, Tile, LEAF_Z, TOP_Z};
use crate::space::zoom::Zoom;

/// A leaf's name length: the bits of its tile grid, which is one world of
/// tiles, so its zoom.
pub const TILE_KEY_DEPTH: u32 = LEAF_Z as u32;

const _: () = assert!(around(TOP_Z) > 1, "every zoom's tile has a name");

/// x in the low bit, y in the high one.
const DIGITS: &str = "0123";

/// A tile index's place in a grid `depth` bits a side. X wraps at the seam; Y
/// flips, since rows count down from the north (pinned by `the_known_tile`).
fn tile_of(idx: IVec, depth: u32) -> (u64, u64) {
    let n = 1i64 << depth;
    let x = (ivec::X(idx) + n / 2).rem_euclid(n) as u64;
    let y = (n / 2 - 1 - ivec::Y(idx)).rem_euclid(n) as u64;
    (x, y)
}

/// Written into a buffer rather than built, so a read allocates no `String`.
pub struct Quadkey {
    depth: u32,
    tx: u64,
    ty: u64,
}

impl fmt::Display for Quadkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in (1..=self.depth).rev() {
            let bit = 1u64 << (i - 1);
            let d = usize::from(self.tx & bit != 0) | usize::from(self.ty & bit != 0) << 1;
            f.write_str(&DIGITS[d..d + 1])?;
        }
        Ok(())
    }
}

pub fn quadkey_of(depth: u32, idx: IVec) -> Quadkey {
    let (tx, ty) = tile_of(idx, depth);
    Quadkey { depth, tx, ty }
}

/// The depth and tile index a name addresses, Z zero; `None` if it is not a name.
pub fn index_of(key: &str) -> Option<(u32, IVec)> {
    let depth = u32::try_from(key.len()).ok()?;
    if depth == 0 || depth > TILE_KEY_DEPTH {
        return None;
    }
    let (mut tx, mut ty) = (0i64, 0i64);
    for (i, c) in key.bytes().enumerate() {
        let d = DIGITS.bytes().position(|d| d == c)? as i64;
        let bit = 1i64 << (depth as usize - 1 - i);
        tx |= (d & 1) * bit;
        ty |= (d >> 1) * bit;
    }
    let n = 1i64 << depth;
    Some((depth, ivec::gather(tx - n / 2, n / 2 - 1 - ty, 0)))
}

/// The bits of the tile grid at `z`, a name's length.
fn depth_at(z: Zoom) -> u32 {
    around(z).trailing_zeros()
}

pub fn tile_name(p: Tile) -> Quadkey {
    quadkey_of(depth_at(p.zoom), p.tile)
}

/// `tile_name`'s inverse; `None` for a zoom the pyramid does not hold.
pub fn tile_of_name(name: &str) -> Option<Tile> {
    let (depth, tile) = index_of(name)?;
    (TOP_Z..=LEAF_Z).find(|&z| depth_at(z) == depth).map(|z| Tile::new(z, tile))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
