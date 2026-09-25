// pxl-spec/space/xy.rs

// Uppercase throughout, as the notation these operations are written in: one
// that holds no state and closes over nothing is named the way a C header
// names its macros. Rust's own answer to a macro like that is a `const fn`,
// which it cases snake, so the notation and the language disagree here and
// the file keeps the notation — suspended once for the file, which is the
// level the choice is made at rather than item by item.
#![allow(non_snake_case)]

use super::pvf3::{ivec, IVec};

/// Drops Z, giving the page/cell key every 2D map surface is keyed by.
///
/// **This is a recurring source of wrong hypotheses — read before theorising
/// about a Z mismatch.** Tiles are 2D: a baked page covers an X/Y footprint
/// and carries elevation as *data* (`Terrain.ele`), never as a third page
/// axis. So every page-keyed map — every layer's pyramid tables, the fetch scheduler's
/// demand keys — normalizes through TO_XY,
/// and `L3_IDX(map_center.i)` must be TO_XY'd before it is compared against
/// one of those keys.
///
/// The trap, and why it is easy to get backward in both directions:
///
/// - It usually cannot bite. `L3_IDX` shifts X/Y/Z together, but real
///   elevations sit far inside one L3 page, so `Z` of `L3_IDX(map_center.i)`
///   is normally **0** and the un-normalized index compares equal anyway.
///   Measured in Bundang: `L3_IDX(map_center.i)` = `[5785, 1836, 0]`, identical
///   to its own TO_XY. A missing TO_XY therefore looks harmless in testing.
/// - When it does bite it is total, not partial, because CHEBYSHEV_DIST is a
///   `max` over |X|,|Y|,|Z|: a single nonzero Z term dominates every real X/Y
///   distance at once, so a radius filter rejects *everything* rather than
///   degrading. That failure mode looks like "nothing loads/renders", which
///   invites blaming the data.
///
/// Consequence for debugging: a blank world is **not** evidence of a Z
/// mismatch — check whether a frame has actually been rendered first
/// (`frameloop="demand"` means an idle scene has stale/empty stats), and
/// confirm the Z term is nonzero before pursuing this at all.
pub fn TO_XY(i: IVec) -> IVec {
    ivec::gather(ivec::X(i), ivec::Y(i), 0)
}

// temp
pub fn WITH_X(x: i64, i: IVec) -> IVec {
    ivec::gather(x, ivec::Y(i), ivec::Z(i))
}
pub fn WITH_Y(y: i64, i: IVec) -> IVec {
    ivec::gather(ivec::X(i), y, ivec::Z(i))
}
pub fn WITH_Z(z: i64, i: IVec) -> IVec {
    ivec::gather(ivec::X(i), ivec::Y(i), z)
}

pub fn CELL_CENTER_U(u_per_cell: f64) -> f64 {
    u_per_cell * 0.5
}
