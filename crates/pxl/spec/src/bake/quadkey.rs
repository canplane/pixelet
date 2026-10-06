// pxl-spec/bake/quadkey.rs

//! A baked tile's object-store name: its 2D key's digits
//! (`space/hierarchy/morton.rs`), two bits a digit, coarsest first. A coarser tile's name prefixes a finer one's over
//! the same ground, and every well-formed name is a tile.

use crate::space::layer::lattice::GROUND;
use core::fmt;

use crate::space::coord::pvf3::ivec::{self, IVec};
use crate::space::coord::world::{around, wrap_index};
use crate::space::coord::zoom::{shamt, Zoom};
use crate::space::hierarchy::morton::{key_of, node_of};
use crate::space::hierarchy::tree::{ancestor_of, first_descendant_of};
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

/// A leaf's name length: one digit a zoom.
pub const TILE_KEY_DEPTH: u32 = LEAF_ZOOM as u32;

const _: () = assert!(around(GROUND.top()) > 1, "every zoom's tile has a name");

/// Written into a buffer rather than built, so a read allocates no `String`.
pub struct Quadkey {
    key: u128,
}

impl fmt::Display for Quadkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = 126 - self.key.trailing_zeros();
        for i in (0..n / 2).rev() {
            let d = ((self.key >> (127 - n + 2 * i)) & 3) as usize;
            f.write_str(&"0123"[d..d + 1])?;
        }
        Ok(())
    }
}

/// The name of the node at `zoom` holding the point `at`, in units; zoom 0's
/// is empty, the prefix of every name.
pub fn node_name(zoom: Zoom, at: [i64; 2]) -> Quadkey {
    Quadkey {
        key: key_of(zoom, ancestor_of(at, shamt(zoom) as u32)),
    }
}

/// The name of the tile at zoom `depth` and index `idx`, X wrapped at the seam.
pub fn quadkey_of(depth: u32, idx: IVec) -> Quadkey {
    let zoom = depth as Zoom;
    node_name(zoom, first_descendant_of([wrap_index(zoom, ivec::X(idx)), ivec::Y(idx)], shamt(zoom) as u32))
}

/// The depth and tile index a name addresses, Z zero; `None` if it is not a name.
pub fn index_of(name: &str) -> Option<(u32, IVec)> {
    let depth = u32::try_from(name.len()).ok()?;
    if depth == 0 || depth > TILE_KEY_DEPTH {
        return None;
    }
    let digits = name.bytes().try_fold(0u128, |key, c| Some((key << 2) | u128::from(c.checked_sub(b'0').filter(|&d| d < 4)?)))?;
    let n = 2 * depth;
    let (_, [x, y]) = node_of::<2>((digits << (127 - n)) | (1 << (126 - n)));
    Some((depth, ivec::gather(x, y, 0)))
}

pub fn tile_name(p: Tile) -> Quadkey {
    quadkey_of(u32::from(p.zoom), p.tile)
}

/// `tile_name`'s inverse; `None` for a zoom the pyramid does not hold.
pub fn tile_of_name(name: &str) -> Option<Tile> {
    let (depth, tile) = index_of(name)?;
    let zoom = depth as Zoom;
    (GROUND.top()..=LEAF_ZOOM).contains(&zoom).then(|| Tile::new(zoom, tile))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
