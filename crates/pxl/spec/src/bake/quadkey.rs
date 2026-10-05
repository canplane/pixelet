// pxl-spec/bake/quadkey.rs

//! A baked page's object-store name: its place in its zoom's page grid as a
//! quadkey, coarsest digit first. A coarser page's name prefixes a finer one's
//! over the same ground, and every well-formed name is a page.

use core::fmt;

use crate::space::indexer::L3;
use crate::space::pvf3::core::IVEC_X_W;
use crate::space::pvf3::ivec::{self, IVec};
use crate::space::pyramid::{pages_around, Page, LEAF_Z, TOP_Z};

/// A leaf's name length: the bits of its page grid, which is one world
/// of pages. It follows the page's size, never the zoom.
pub const PAGE_KEY_DEPTH: u32 = IVEC_X_W - L3.shamt() as u32;

const _: () = assert!(pages_around(TOP_Z) > 1, "every zoom's page has a name");

/// x in the low bit, y in the high one.
const DIGITS: &str = "0123";

/// A page index's place in a grid `depth` bits a side. X wraps at the seam; Y
/// flips, since rows count down from the north (pinned by `the_known_page`).
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

/// The depth and page index a name addresses, Z zero; `None` if it is not a name.
pub fn index_of(key: &str) -> Option<(u32, IVec)> {
    let depth = u32::try_from(key.len()).ok()?;
    if depth == 0 || depth > PAGE_KEY_DEPTH {
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

/// The bits of the page grid at `z`, a name's length.
fn depth_at(z: u32) -> u32 {
    pages_around(z).trailing_zeros()
}

pub fn page_name(p: Page) -> Quadkey {
    quadkey_of(depth_at(p.z), p.page)
}

/// `page_name`'s inverse; `None` for a zoom the pyramid does not hold.
pub fn page_of_name(name: &str) -> Option<Page> {
    let (depth, page) = index_of(name)?;
    (TOP_Z..=LEAF_Z).find(|&z| depth_at(z) == depth).map(|z| Page::new(z, page))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
