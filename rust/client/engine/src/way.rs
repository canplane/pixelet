// pxl-engine/way.rs

//! wgsl-port: the ways off the ground, as a cell reads them. A brick's fill makes each
//! column's spans once, on the CPU, and keeps them (pxl-spec way.rs `column`); the query, on
//! the CPU and on the GPU, reads a cell off those spans (`way_cell`, clipmap.rs `way_at`,
//! pxl-render way.wgsl). A cell holds what its own spans write there (`at`); ground beside the
//! air the spans leave takes that way's palette index, read from the spans of the columns
//! beside it (`way_cell`), so a brick's fill and a reader read its ring alike.

use crate::ground;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};
use pxl_spec::value;
use pxl_spec::way::{fill_range, fills, AIR_OF, NOTHING};

/// Whether run `v` sorts at or after `best`, by its low end, then its high.
fn at_or_after(v: [i32; 2], best: [i32; 2]) -> bool {
    v[0] > best[0] || (v[0] == best[0] && v[1] >= best[1])
}

/// The spans of a brick's columns and of the ring around it, read in place as its fill keeps
/// them (pxl-engine fill.rs `spans`, clipmap.rs `KeptSpans`): column `c`'s face cell and how many
/// spans it has (`SpanList`), and its `j`-th span with its value.
pub trait SpanTable {
    fn header(&self, c: [i32; 2]) -> (i32, u32);
    fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32);
}

/// What the ways write in cell `z` at zoom `cell_zoom` of column `c`, whose `n` spans are
/// kept in `k`. A deck's slab or its pier comes first: where two share the cell, the higher
/// keeps it, and a slab goes before its pier. Otherwise it is the first air over the cell
/// (`AIR_OF` over its way's palette index), or else `NOTHING`.
fn at(k: &impl SpanTable, c: [i32; 2], n: u32, cell_zoom: Zoom, z: i32) -> u32 {
    let (mut best, mut palette_idx, mut air) = ([i32::MIN, i32::MIN], NOTHING, NOTHING);
    let mut j = 0;
    while j < n {
        let (run, v) = k.span(c, j);
        if fills(cell_zoom, run, z) {
            if v & AIR_OF != 0 {
                if air == NOTHING {
                    air = v;
                }
            } else if at_or_after(run, best) {
                (best, palette_idx) = (run, v);
            }
        }
        j += 1;
    }
    if palette_idx != NOTHING {
        palette_idx
    } else {
        air
    }
}

/// The offset of the `k`-th of a cell and the six around it, the cell's own first: then the
/// row under it, the column before it, the cell under it, the cell over it, the column after
/// it and the row over it. This is the order of the grid's cells.
fn near(k: u32) -> [i32; 3] {
    // Its axis (y, x, then z) and its side.
    let (i, s) = if k < 4 {
        (k.wrapping_sub(1), -1)
    } else {
        (6 - k, 1)
    };
    let on = |axis: u32| if k != 0 && i == axis { s } else { 0 };
    [on(1), on(0), on(2)]
}

/// Where the ways may fill a node along z, at the zooms from `coarsest` to `near` that a
/// reader asks within it, where what they fill under it lies in the leaf cells `[lo, hi)`
/// (`ground::Extent`, with a bore's air counted as none): from its lowest to past its
/// highest, in leaf cells. A cell fills where more than half of it is filled (`fills`). At a
/// zoom whose cell is `s` leaf cells, the slab runs from the first cell more than half over
/// `lo` to past the last more than half under `hi`; at the leaves or finer it is `[lo, hi)`
/// itself, a cell filling as its leaf does. It is nothing (`ground::EMPTY`) where the ways
/// fill nothing.
///
/// The node's bottom and top are whole cells of every zoom finer than it, so none of its
/// cells fills exactly where it does not meet the finest zoom's slab (`ground::meets`), and a
/// coarser zoom's cell, more than twice as large, fills less still within it.
pub fn slab([lo, hi]: [i32; 2], [coarsest, near]: [Zoom; 2]) -> [i32; 2] {
    if lo >= hi {
        return ground::EMPTY;
    }
    let [mut under, mut over] = ground::EMPTY;
    let mut z = coarsest;
    while z <= near {
        let (a, b) = if z < LEAF_CELL_ZOOM {
            let k = LEAF_CELL_ZOOM - z;
            let half = 1 << (k - 1);
            ((lo + half) >> k << k, -((half - hi) >> k << k))
        } else {
            (lo, hi)
        };
        under = if a < under { a } else { under };
        over = if b > over { b } else { over };
        z += 1;
    }
    [under, over]
}

/// The column that the `m`-th of `near` reads beside column `c`, and the cell it reads there
/// for cell `z`; for the cells under and over, the column is `c` itself.
fn beside(c: [i32; 2], m: u32, z: i32) -> ([i32; 2], i32) {
    let d = near(m);
    ([c[0] + d[0], c[1] + d[1]], z + d[2])
}

/// What the ways write in cell `z` of column `c` at zoom `cell_zoom`, where its spans and those
/// beside it are kept in `k` (`at` of its own). A deck's slab or its pier comes first, or else a
/// bore's air (`value::VOID`). Under its face cell, where a cave may take it (pxl-spec
/// `ground::face_cell`), it is otherwise the way's palette index wherever a bore's air lies beside
/// it, under or over it or across one of its four faces, the first in `near`'s order; so a cave
/// never opens into a way, whatever carves the ground. Otherwise it is `NOTHING`. It is read from
/// the spans in place, the column's own first, and a cell the column writes reads nothing beside
/// it.
pub fn way_cell(k: &impl SpanTable, c: [i32; 2], cell_zoom: Zoom, z: i32) -> u32 {
    let (face, n) = k.header(c);
    let v = at(k, c, n, cell_zoom, z);
    if v != NOTHING {
        return if v & AIR_OF != 0 { value::VOID } else { v };
    }
    let mut m = 1;
    while z < face && m < 7 {
        let (b, zb) = beside(c, m, z);
        let v = at(k, b, k.header(b).1, cell_zoom, zb);
        if v != NOTHING && v & AIR_OF != 0 {
            return v & !AIR_OF;
        }
        m += 1;
    }
    NOTHING
}

/// The leaf cells between which column `c`'s cells at zoom `cell_zoom` hold what the ways
/// write there (`way_cell`), where its ground's top is `top` leaf cells: from the lowest to
/// past the highest, or `ground::EMPTY` where they write none. It covers its own spans' cells
/// (`fill_range`), with a bore's air only as far as the top, above which a cell is air
/// whatever writes it; and under its face cell, the cells that a bore's air beside it lines
/// where no slab or pier of that column fills the cell read (`bare`). A lined cell is under
/// the face's cell, so under the top. So the result is exactly the hull of the cells
/// `way_cell` reads something in, read from the spans.
pub fn lined_hull(k: &impl SpanTable, c: [i32; 2], cell_zoom: Zoom, top: i32) -> [i32; 2] {
    let s = 1i32 << (LEAF_CELL_ZOOM - cell_zoom);
    let (face, n) = k.header(c);
    let [mut lo, mut hi] = ground::EMPTY;
    let mut j = 0;
    while j < n {
        let (run, v) = k.span(c, j);
        let [first, last] = fill_range(cell_zoom, run);
        let end = if v & AIR_OF != 0 {
            ((last + 1) * s).min(top)
        } else {
            (last + 1) * s
        };
        if first <= last && first * s < end {
            (lo, hi) = (lo.min(first * s), hi.max(end));
        }
        j += 1;
    }
    // A column with no face lines nothing, and has no cell under its face to name.
    let mut m = if face == i32::MIN { 7 } else { 1 };
    while m < 7 {
        let (b, dz) = beside(c, m, 0);
        let nb = k.header(b).1;
        let mut j = 0;
        while j < nb {
            let (run, v) = k.span(b, j);
            // Air read at `z + dz` lines `z`, under the face cell, where no slab or pier of its
            // column fills it.
            let [first, last] = fill_range(cell_zoom, run);
            let last = last.min(face - 1 + dz);
            let (from, to) = (
                bare(k, b, nb, cell_zoom, [first, last], 1),
                bare(k, b, nb, cell_zoom, [last, first], -1),
            );
            if v & AIR_OF != 0 && from <= last {
                (lo, hi) = (lo.min((from - dz) * s), hi.max((to - dz + 1) * s));
            }
            j += 1;
        }
        m += 1;
    }
    [lo, hi]
}

/// The first cell from `from` toward `to`, a `step` at a time, that no slab or pier of
/// column `b`'s `n` spans fills, so that its air is read as air (`at`); past `to` where they
/// fill every one. Each step passes one of them, so it takes no more than one step a span.
fn bare(
    k: &impl SpanTable,
    b: [i32; 2],
    n: u32,
    cell_zoom: Zoom,
    [from, to]: [i32; 2],
    step: i32,
) -> i32 {
    let (mut z, mut moved) = (from, true);
    while moved && (z - to) * step <= 0 {
        moved = false;
        let mut j = 0;
        while j < n {
            let (run, v) = k.span(b, j);
            let [first, last] = fill_range(cell_zoom, run);
            if v & AIR_OF == 0 && first <= z && z <= last {
                (z, moved) = (if step > 0 { last + 1 } else { first - 1 }, true);
            }
            j += 1;
        }
    }
    z
}

#[cfg(test)]
mod tests {
    use super::*;
    use pxl_spec::ground::{cells, face_cell, face_height};
    use pxl_spec::value::PIER;

    /// A column: its face cell, its spans and how many it holds.
    type Column = (i32, [([i32; 2], u32); 4], u32);

    /// A column and the four beside it, each a face cell and its spans.
    struct Five([Column; 5]);

    impl Five {
        fn of(&self, c: [i32; 2]) -> &Column {
            let k = match c {
                [0, 0] => 0,
                [0, -1] => 1,
                [-1, 0] => 2,
                [1, 0] => 3,
                _ => 4,
            };
            &self.0[k]
        }
    }

    impl SpanTable for Five {
        fn header(&self, c: [i32; 2]) -> (i32, u32) {
            let (face, _, n) = self.of(c);
            (*face, *n)
        }
        fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32) {
            self.of(c).1[j as usize]
        }
    }

    /// `lined_hull` is the hull of the cells `way_cell` writes, a cell at a time, with a bore's
    /// air held to the top. It is tried for every way a slab, a pier or air can lie over another,
    /// beside it, or over and under it, at zooms from the leaves to three coarser.
    #[test]
    fn bound_is_hull() {
        let mut seed = 0x2545_f491_u32;
        let mut next = |m: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % m
        };
        for _ in 0..20_000 {
            let cell_zoom = LEAF_CELL_ZOOM - next(4);
            let s = 1i32 << (LEAF_CELL_ZOOM - cell_zoom);
            let top = next(120) as i32 - 60;
            let face = face_cell(
                cell_zoom,
                face_height(cell_zoom, cells(cell_zoom, top as f32)),
            );
            let mut five = [(0, [([0, 0], 0); 4], 0); 5];
            for (k, col) in five.iter_mut().enumerate() {
                let n = next(5);
                for j in 0..n as usize {
                    let lo = next(140) as i32 - 70;
                    let v = match next(3) {
                        0 => AIR_OF | 3,
                        1 => 7,
                        _ => PIER,
                    };
                    col.1[j] = ([lo, lo + 1 + next(30) as i32], v);
                }
                col.2 = n;
                col.0 = if k == 0 && next(8) == 0 {
                    i32::MIN
                } else {
                    face
                };
            }
            let k = Five(five);
            let mut hull = ground::EMPTY;
            for z in -200 / s..200 / s {
                let v = way_cell(&k, [0, 0], cell_zoom, z);
                if v == NOTHING {
                    continue;
                }
                let end = if v == value::VOID {
                    ((z + 1) * s).min(top)
                } else {
                    (z + 1) * s
                };
                if z * s < end {
                    hull = [hull[0].min(z * s), hull[1].max(end)];
                }
            }
            assert_eq!(
                lined_hull(&k, [0, 0], cell_zoom, top),
                hull,
                "{:?} at {cell_zoom}, top {top}",
                k.0
            );
        }
    }
}
