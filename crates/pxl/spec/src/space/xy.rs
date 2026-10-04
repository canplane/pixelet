// pxl-spec/space/xy.rs

// Uppercase as the notation these macro-like helpers are written in.
#![allow(non_snake_case)]

use super::pvf3::{ivec, IVec};

/// Drops Z: pages are keyed by X/Y only, so `L3_IDX(map_center.i)` must pass
/// through this before it is compared with a page key. A nonzero Z there
/// dominates a Chebyshev distance and rejects everything.
pub fn TO_XY(i: IVec) -> IVec {
    ivec::gather(ivec::X(i), ivec::Y(i), 0)
}

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
