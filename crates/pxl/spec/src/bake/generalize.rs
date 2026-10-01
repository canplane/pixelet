// pxl-spec/bake/generalize.rs

//! How the ways on a page survive being folded up the pyramid: the one
//! place the fold tells a line from an area.
//!
//! The ground's channels fold by majority (bake/fold.rs), which is right for
//! an area — what fills half a cell is the cell — and wrong for a way. A way
//! is a line rasterized into cells, a few of them across, and a level up a
//! diagonal two cells wide fills about half of each block it crosses: it wins
//! some votes and loses the next, and the road comes out a dashed line, then
//! scattered dots, then nothing. A map does not draw a road that way. It keeps
//! a road continuous at a width the eye can follow and drops whole classes
//! as the scale grows — the cartographic generalization every road map does,
//! OpenStreetMap Carto's motorways shown from its smallest zooms and its
//! footways from its largest. So a way that may still be shown at a level
//! keeps its cell whenever any of the cells under it holds it, the one of
//! higher precedence where several do (Carto's `z_order`, as the ground
//! paints them), and a class past its last level is folded as an area is.
//!
//! The last levels are pixelet's, said here rather than derived: tuned on the
//! screen against the view's own levels, where a level-k cell is 2^k units,
//! about 2.4 · 2^k metres at the equator.

use crate::cell::cube::cube_types::transport as t;

/// The coarsest level a transport type is still shown at as a line, and its
/// precedence where lines meet; none for a type that is an area — a parking
/// lot, a runway — which folds by majority like any other.
pub fn line(kind: u8) -> Option<(u32, u32)> {
    // (last level, precedence: Carto's z_order)
    Some(match kind {
        k if k == t::rail_main => (6, 440),
        k if k == t::rail_subway => (4, 420),
        k if k == t::road_motorway => (6, 380),
        k if k == t::road_trunk => (6, 370),
        k if k == t::road_primary => (5, 360),
        k if k == t::road_secondary => (4, 350),
        k if k == t::road_tertiary => (3, 340),
        k if k == t::road_default => (2, 330),
        k if k == t::road_service => (2, 150),
        k if k == t::track => (1, 110),
        k if k == t::pedestrian => (1, 100),
        _ => return None,
    })
}

/// The way four cells fold into at `level`, if any of them holds one still
/// shown there: the one of highest precedence.
pub fn fold_line(four: [u8; 4], level: u32) -> Option<u8> {
    four.into_iter().filter_map(|k| line(k).filter(|&(last, _)| level <= last).map(|(_, z)| (z, k))).max().map(|(_, k)| k)
}
