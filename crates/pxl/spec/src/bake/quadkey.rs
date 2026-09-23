// pxl-spec/bake/quadkey.rs

//! What a baked page is called in the object store.
//!
//! A page's name is its **quadkey**: the Web Mercator tile key at the zoom the
//! page is held at, one base-4 digit a zoom, coarsest digit first. That is the
//! whole name. The level is not written beside it, because the number of
//! digits is the level ([`zoom_of_level`]), and neither is the page index,
//! because the digits are it.
//!
//! Two properties follow, and they are why a name has this shape rather than
//! the `<level>/<page index>` it had before:
//!
//! - **A coarser page's name is a prefix of a finer one's over the same
//!   ground.** One prefix in the store is therefore one region at every level
//!   it is held at, and splitting the store later is a prefix match rather
//!   than a second re-keying.
//! - **Every well-formed name is a page.** The old shape admitted any decimal
//!   of twenty digits under any level of two, so the store's key space was
//!   vastly larger than its address space and the Worker carried a pattern to
//!   sieve the difference. Here the two are the same set.
//!
//! The zoom is the caller's, not a level: terrain holds a page at several
//! levels, POI only at the leaf, and a reverse-geocode group is the page
//! `GROUP_SHAMT` coarser. Each says which zoom it means and reads the answer
//! back in its own words.

use core::fmt;

use crate::bake::constant::LEVEL0_ZOOM;
use crate::space::pvf3::ivec::{self, IVec};

/// A quadkey's digits, in the order a quadrant is numbered: x in the low bit,
/// y in the high one.
const DIGITS: &str = "0123";

/// A page index's tile at a zoom.
///
/// X is the lattice's cyclic lane, so an index past the seam wraps rather than
/// falling outside the world. Y is a plain signed extent about the equator
/// while a tile's row is counted down from the north, and that flip is the one
/// place this file can be silently wrong: it is what `the_known_page` pins.
fn tile_of(idx: IVec, zoom: u32) -> (u64, u64) {
    let n = 1i64 << zoom;
    let x = (ivec::X(idx) + n / 2).rem_euclid(n) as u64;
    let y = (n / 2 - 1 - ivec::Y(idx)).rem_euclid(n) as u64;
    (x, y)
}

/// A page's name, written where it is asked for rather than built. The kernel
/// names an object on every read it starts, and a name that is a `String` is a
/// heap block the WASM module pays for and hands straight back.
pub struct Quadkey {
    zoom: u32,
    tx: u64,
    ty: u64,
}

impl fmt::Display for Quadkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in (1..=self.zoom).rev() {
            let bit = 1u64 << (i - 1);
            let d = usize::from(self.tx & bit != 0) | usize::from(self.ty & bit != 0) << 1;
            f.write_str(&DIGITS[d..d + 1])?;
        }
        Ok(())
    }
}

/// The name of the page `idx` addresses at `zoom`.
pub fn quadkey_of(zoom: u32, idx: IVec) -> Quadkey {
    let (tx, ty) = tile_of(idx, zoom);
    Quadkey { zoom, tx, ty }
}

/// The zoom and page index a name addresses, or `None` if it is not a name.
///
/// Z is zero: a baked page covers an X/Y footprint and carries elevation as
/// data rather than as a third page axis (`space/xy.rs`).
pub fn index_of(key: &str) -> Option<(u32, IVec)> {
    let zoom = u32::try_from(key.len()).ok()?;
    if zoom == 0 || zoom > LEVEL0_ZOOM {
        return None;
    }
    let (mut tx, mut ty) = (0i64, 0i64);
    for (i, c) in key.bytes().enumerate() {
        let d = DIGITS.bytes().position(|d| d == c)? as i64;
        let bit = 1i64 << (zoom as usize - 1 - i);
        tx |= (d & 1) * bit;
        ty |= (d >> 1) * bit;
    }
    let n = 1i64 << zoom;
    Some((zoom, ivec::gather(tx - n / 2, n / 2 - 1 - ty, 0)))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
