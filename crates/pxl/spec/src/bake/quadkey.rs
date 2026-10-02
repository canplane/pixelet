// pxl-spec/bake/quadkey.rs

//! A baked page's object-store name: its Web Mercator quadkey at the page's
//! zoom, coarsest digit first. A coarser page's name prefixes a finer one's
//! over the same ground, and every well-formed name is a page.

use core::fmt;

use crate::space::pvf3::ivec::{self, IVec};
use crate::space::pyramid::{zoom_of_level, LevelPage, PAGE_ZOOM, TOP_LEVEL};

/// x in the low bit, y in the high one.
const DIGITS: &str = "0123";

/// A page index's tile at a zoom. X wraps at the seam; Y flips, since tile
/// rows count down from the north (pinned by `the_known_page`).
fn tile_of(idx: IVec, zoom: u32) -> (u64, u64) {
    let n = 1i64 << zoom;
    let x = (ivec::X(idx) + n / 2).rem_euclid(n) as u64;
    let y = (n / 2 - 1 - ivec::Y(idx)).rem_euclid(n) as u64;
    (x, y)
}

/// Written into a buffer rather than built, so a read allocates no `String`.
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

pub fn quadkey_of(zoom: u32, idx: IVec) -> Quadkey {
    let (tx, ty) = tile_of(idx, zoom);
    Quadkey { zoom, tx, ty }
}

/// The zoom and page index a name addresses, Z zero; `None` if it is not a name.
pub fn index_of(key: &str) -> Option<(u32, IVec)> {
    let zoom = u32::try_from(key.len()).ok()?;
    if zoom == 0 || zoom > PAGE_ZOOM {
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

pub fn page_name(p: LevelPage) -> Quadkey {
    quadkey_of(zoom_of_level(p.level), p.page)
}

/// `page_name`'s inverse; `None` for a level the pyramid does not hold.
pub fn page_of_name(name: &str) -> Option<LevelPage> {
    let (zoom, page) = index_of(name)?;
    let level = PAGE_ZOOM - zoom;
    (level <= TOP_LEVEL).then(|| LevelPage::new(level, page))
}

#[cfg(test)]
#[path = "quadkey.test.rs"]
mod test;
