// pxl-spec/ground.rs

//! wgsl-port: the ground's cells as a tile holds them and the rules a column reads them by,
//! which the bake and the client both run: a cell's elevation from the tile's samples
//! (`ele_at`), its four palette indices and its own (`own`), and its top and face in cells of a
//! zoom (`top`, `face_ele`). The ground's column as a descent asks it is pxl-engine's
//! (ground.rs `node`).
//!
//! A palette index and an elevation are stored as a byte and an i16 and read into a word here,
//! since WGSL has no narrower integer.
//!
//! A cell's elevation from the samples (`ele_at`) is exact, in integers, so the CPU and a GPU
//! draw it alike whatever they fuse or reorder.

use crate::share::{eighths, FULL};
use crate::space::coord::zoom::{Zoom, HEIGHT_BITS, LEAF_CELL_ZOOM, LEAF_ZOOM, UNIT_ZOOM};
use crate::space::layer::lattice::TILE_CELLS;
use crate::value;

pub const UNKNOWN: u32 = value::UNKNOWN;

/// The inner elevation samples across one leaf tile. A leaf tile is about twice as many
/// meters wide as it has units on a side, and the source's spacing is about 30 m, taken as
/// 32 m.
pub const TILE_ELE_SPAN: u32 = 1 << (UNIT_ZOOM - LEAF_ZOOM + 1 - 5);

/// Two samples of padding give the bicubic a 4×4 kernel at every edge texel,
/// and let the ground's elevation 32 cells past the tile be read without the
/// neighboring tile resident.
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = TILE_ELE_SPAN + 2 * ELE_PAD;

/// Sample (x, y) of a padded elevation grid, rows from the south.
pub const fn ele_idx(x: u32, y: u32) -> usize {
    (y * ELE_PADDED_SPAN + x) as usize
}

/// The elevation the surface draws at the center of a tile's cell `(x, y)`, in leaf cells, from
/// its padded elevation samples `ele` (`ele_idx`). It is the one producer of a cell's
/// elevation, which the core's fill and the bake both run (fill.rs `columns`): Catmull-Rom over
/// the 4 × 4 samples around the center, rounded half to even and held to their range, so a
/// bound over the surface is the samples' own.
///
/// It is exact, its bits counted. A sample is an `i16`. A center lies an odd number of
/// `CELL_STEPS`ths of a sample past one (`window`), where each weight is an integer over
/// `1 << WEIGHT_BITS` (`catmull_rom`), under 2^16 in size. A sample times a weight is under
/// 2^31. A row's sum, with its weights' sizes summing under 1.23 · 2^16, is under 2^31.3. A
/// row times a weight, the four summed, is under 2^47.6 and fits an `i64`, whose high word is
/// the elevation's floor and whose low word is the fraction to round. Filling a brick costs, for
/// each column, 16 products of 32 bits and 4 of 64.
pub fn ele_at(ele: impl Fn(usize) -> i32, [x, y]: [i32; 2]) -> i32 {
    let (x0, mx) = window(x);
    let (y0, my) = window(y);
    let (wx, wy) = (catmull_rom(mx), catmull_rom(my));
    let (mut sum, mut lo, mut hi) = (0i64, i32::MAX, i32::MIN);
    let mut j = 0;
    while j < 4 {
        let py = clamp_idx(y0 + j);
        let mut row = 0i64;
        let mut i = 0;
        while i < 4 {
            let v = ele(ele_idx(clamp_idx(x0 + i), py));
            lo = lo.min(v);
            hi = hi.max(v);
            row += i64::from(v * wx[i as usize]);
            i += 1;
        }
        sum += row * i64::from(wy[j as usize]);
        j += 1;
    }
    let one = 1i64 << (2 * WEIGHT_BITS);
    let (floor, frac) = (sum >> (2 * WEIGHT_BITS), sum & (one - 1));
    let up = 2 * frac > one || 2 * frac == one && floor & 1 == 1;
    ((floor + i64::from(up)) as i32).clamp(lo, hi)
}

/// The bits a palette index takes in a cell's word, four to the word.
pub const PALETTE_BITS: u32 = 8;

/// A cell's word's four palette indices, `dx + 2·dy`.
pub fn asset_ids(word: u32) -> [u32; 4] {
    let m = (1 << PALETTE_BITS) - 1;
    [
        word & m,
        word >> PALETTE_BITS & m,
        word >> (2 * PALETTE_BITS) & m,
        word >> (3 * PALETTE_BITS),
    ]
}

/// Four palette indices as a cell's word.
pub fn word([a, b, c, d]: [u32; 4]) -> u32 {
    a | b << PALETTE_BITS | c << (2 * PALETTE_BITS) | d << (3 * PALETTE_BITS)
}

/// A cell's own look at `cell_zoom`: the majority of the four it folds, as the
/// bake folds a tile's (pxl-spec bake/fold.rs `fold_ground`).
pub fn own(w: u32, cell_zoom: Zoom) -> u32 {
    majority(asset_ids(w), cell_zoom)
}

/// The steps of half a cell that an elevation sample spans, since a cell's center lies on a
/// half cell.
pub const CELL_STEPS: i32 = 2 * (TILE_CELLS / TILE_ELE_SPAN) as i32;

/// The bits of a weight's fraction (`catmull_rom`): its denominator is
/// `2 · CELL_STEPS³`.
pub const WEIGHT_BITS: u32 = 1 + 3 * CELL_STEPS.trailing_zeros();

/// The first of the four samples the surface reads along an axis about
/// cell `g`'s center, and how far past the second the center falls, in
/// `CELL_STEPS`ths of a sample, odd: the center `g + ½` lies at
/// `(g + ½) / (CELL_STEPS / 2) + ELE_PAD − ½` samples.
pub fn window(g: i32) -> (i32, i32) {
    let s = 2 * g + 1 + CELL_STEPS * ELE_PAD as i32 - CELL_STEPS / 2;
    ((s >> CELL_STEPS.trailing_zeros()) - 1, s & (CELL_STEPS - 1))
}

/// A sample index on the padded grid: past its edge, the edge sample stands.
pub fn clamp_idx(i: i32) -> u32 {
    i.clamp(0, ELE_PADDED_SPAN as i32 - 1) as u32
}

/// Catmull-Rom's four weights at `m / CELL_STEPS` past the second sample,
/// each times `1 << WEIGHT_BITS`, an integer: `−½t³ + t² − ½t`,
/// `1½t³ − 2½t² + 1`, `−1½t³ + 2t² + ½t` and `½t³ − ½t²` over the
/// denominator.
pub fn catmull_rom(m: i32) -> [i32; 4] {
    let n = CELL_STEPS;
    [
        -m * (n - m) * (n - m),
        3 * m * m * m - 5 * n * m * m + 2 * n * n * n,
        -3 * m * m * m + 4 * n * m * m + n * n * m,
        m * m * (m - n),
    ]
}

const _: () = assert!(
    (CELL_STEPS as u32).is_power_of_two()
        && 2 * CELL_STEPS * CELL_STEPS * CELL_STEPS == 1 << WEIGHT_BITS,
    "a weight's denominator is a power of two"
);
const _: () = assert!(
    2 * WEIGHT_BITS == u32::BITS,
    "an elevation's sum's high word is its floor"
);

/// The value filling more than half of a 2×2 (or 2×2×2) group, `v[dx + 2·dy +
/// 4·dz]`; otherwise the tie child's. The tie corner alternates with the
/// parent cell's zoom, since a fixed corner drifts the map half a coarse cell a step.
pub fn majority<T: Copy + Eq, const N: usize>(v: [T; N], parent: Zoom) -> T {
    let tie = v[if parent & 1 == 1 { 0 } else { N - 1 }];
    let cnt = |x: T| {
        let (mut n, mut k) = (0, 0);
        while k < N {
            n += usize::from(v[k] == x);
            k += 1;
        }
        n
    };
    let mut half: Option<T> = None;
    let mut k = 0;
    while k < N {
        let x = v[k];
        let n = cnt(x);
        if 2 * n > N {
            return x;
        }
        if 2 * n == N {
            match half {
                Some(h) if h != x => return tie,
                _ => half = Some(x),
            }
        }
        k += 1;
    }
    match half {
        Some(h) => h,
        None => tie,
    }
}

/// An elevation in leaf cells as cells of `cell_zoom` (`top`).
pub fn cells(cell_zoom: Zoom, h: f32) -> i32 {
    top(cell_zoom, h).0
}

/// An elevation in leaf cells as cells of `cell_zoom`, and the share of the top cell it fills,
/// in `share::FULL`'s units. Finer than the leaves, the elevation is shifted and every cell is
/// whole. Coarser, the elevation is the mean in cells: its top is the cell it ends in where it
/// fills an eighth of that cell or more (`share::eighths`), keeping the share filled there,
/// and otherwise the cell under it, whole. So a coarse column's ground is the volume its mean
/// elevation fills, as a cell of it is drawn: whole under its top, with its top a box of its
/// share's volume.
pub fn top(cell_zoom: Zoom, h: f32) -> (i32, u32) {
    let d = cell_zoom as i32 - LEAF_CELL_ZOOM as i32;
    if d >= 0 {
        return ((h as i32) << d, FULL);
    }
    let q = h / (1 << -d) as f32;
    let under = libm::floorf(q);
    let part = ((q - under) * FULL as f32) as u32;
    if eighths(part) == 0 {
        (under as i32, FULL)
    } else {
        (under as i32 + 1, part)
    }
}

/// How many cells of `cell_zoom` tall a leaf cell is: one where a cell folds the leaves.
pub fn leaf_cells(cell_zoom: Zoom) -> i32 {
    1 << (cell_zoom as i32 - LEAF_CELL_ZOOM as i32).max(0)
}

/// The face of a top `top` cells of `cell_zoom` high, in leaf cells, as the grid draws it.
/// Finer than the leaves, it is the leaf cell the top's cell lies in, rounded down under sea
/// level as over it, as a cell's index is. Coarser, it is the mean as its cells round it
/// (pxl-render world_rule.wgsl `face_ele`).
pub fn face_ele(cell_zoom: Zoom, top: i32) -> i32 {
    let d = cell_zoom as i32 - LEAF_CELL_ZOOM as i32;
    if d >= 0 {
        top >> d
    } else {
        top << -d
    }
}

/// The face cell of a column whose face is `face` leaf cells high, in cells of `cell_zoom`:
/// the first cell of the ground under the face, which a cave may take (`Cell::opens`) and in
/// which a way's bore is lined (pxl-engine `way::way_cell`).
pub fn face_cell(cell_zoom: Zoom, face: i32) -> i32 {
    cells(cell_zoom, face as f32) - leaf_cells(cell_zoom)
}

const _: () = assert!(
    LEAF_CELL_ZOOM == UNIT_ZOOM,
    "a span's leaf cell is the world's unit"
);
const _: () = assert!(
    -(1i32 << (HEIGHT_BITS - 1)) == i16::MIN as i32
        && (1i32 << (HEIGHT_BITS - 1)) - 1 == i16::MAX as i32,
    "the world's z, in leaf cells, is i16's range"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_top_eighths() {
        // Cells of eight leaf cells.
        let cell_zoom = LEAF_CELL_ZOOM - 3;
        assert_eq!(top(cell_zoom, 16.0), (2, FULL));
        assert_eq!(top(cell_zoom, 17.0), (3, FULL / 8));
        assert_eq!(top(cell_zoom, 23.0), (3, 7 * FULL / 8));
        assert_eq!(top(cell_zoom, -1.0), (0, 7 * FULL / 8));
        // An eighth of a cell of 64 is 8 leaf cells: under it, nothing.
        assert_eq!(top(LEAF_CELL_ZOOM - 6, 7.0), (0, FULL));
        assert_eq!(top(LEAF_CELL_ZOOM - 6, 8.0), (1, FULL / 8));
        // At the leaves, every cell whole.
        assert_eq!(top(LEAF_CELL_ZOOM, 5.0), (5, FULL));
    }

    /// A face past the leaves is the leaf cell its top's cell lies in,
    /// rounded down under sea level as over it; under the leaves, the cells
    /// scaled.
    #[test]
    fn face_ele_floors() {
        let cell_zoom = LEAF_CELL_ZOOM + 2;
        assert_eq!(face_ele(cell_zoom, 8), 2);
        assert_eq!(face_ele(cell_zoom, 9), 2);
        assert_eq!(face_ele(cell_zoom, -8), -2);
        assert_eq!(face_ele(cell_zoom, -9), -3);
        assert_eq!(face_ele(cell_zoom, -1), -1);
        assert_eq!(face_ele(LEAF_CELL_ZOOM - 3, -5), -40);
    }
}
