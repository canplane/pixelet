// pxl-spec/bake/quadkey.rs

//! A baked tile's object-store name: its 2D key (`space/morton.rs`), two bits
//! a digit, coarsest first. A coarser tile's name prefixes a finer one's over
//! the same ground, and every well-formed name is a tile.

use core::fmt;

use crate::space::hierarchy::{ancestor_of, first_descendant_of};
use crate::space::morton;
use crate::space::pvf3::ivec::{self, IVec};
use crate::space::pyramid::{Tile, LEAF_Z, TOP_Z};
use crate::space::zoom::{around, shamt, wrap_index, Zoom};

/// A leaf's name length: one digit a zoom.
pub const TILE_KEY_DEPTH: u32 = LEAF_Z as u32;

const _: () = assert!(around(TOP_Z) > 1, "every zoom's tile has a name");

/// Written into a buffer rather than built, so a read allocates no `String`.
pub struct Quadkey {
    depth: u32,
    key: u64,
}

impl fmt::Display for Quadkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in (0..self.depth).rev() {
            let d = ((self.key >> (2 * i)) & 3) as usize;
            f.write_str(&"0123"[d..d + 1])?;
        }
        Ok(())
    }
}

/// The name of the tile at zoom `depth` and index `idx`, X wrapped at the seam.
pub fn quadkey_of(depth: u32, idx: IVec) -> Quadkey {
    let zoom = depth as Zoom;
    let at = first_descendant_of([wrap_index(zoom, ivec::X(idx)), ivec::Y(idx)], shamt(zoom) as u32);
    Quadkey { depth, key: morton::key(zoom, at) }
}

/// The depth and tile index a name addresses, Z zero; `None` if it is not a name.
pub fn index_of(name: &str) -> Option<(u32, IVec)> {
    let depth = u32::try_from(name.len()).ok()?;
    if depth == 0 || depth > TILE_KEY_DEPTH {
        return None;
    }
    let key = name.bytes().try_fold(0u64, |key, c| Some((key << 2) | u64::from(c.checked_sub(b'0').filter(|&d| d < 4)?)))?;
    let zoom = depth as Zoom;
    let [x, y] = ancestor_of(morton::base(zoom, key), shamt(zoom) as u32);
    Some((depth, ivec::gather(x, y, 0)))
}

pub fn tile_name(p: Tile) -> Quadkey {
    quadkey_of(u32::from(p.zoom), p.tile)
}

/// `tile_name`'s inverse; `None` for a zoom the pyramid does not hold.
pub fn tile_of_name(name: &str) -> Option<Tile> {
    let (depth, tile) = index_of(name)?;
    let zoom = depth as Zoom;
    (TOP_Z..=LEAF_Z).contains(&zoom).then(|| Tile::new(zoom, tile))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
