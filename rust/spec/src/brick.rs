// pxl-spec/brick.rs

//! A clip level's brick as its fill reads it, which the bake reads its tiles'
//! lines by too (bake/tile/terrain.rs `drawn`): its side (`BRICK`), its cells' order
//! (`idx_of`), the bins its tile's ways are cut into and the words its lines are laid in
//! (`WAYS_*`, `LINES_*`), and its columns as the ways' one rule asks them (`Brick`). The clip
//! levels that hold bricks are pxl-engine's (clipmap.rs).

use crate::ground;
use crate::space::coord::world::wrap_idx;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::area;
use crate::space::layer::area::Shard;
use crate::space::layer::lattice;
use crate::space::layer::lattice::TILE_CELLS;
use crate::value::VOID;
use crate::way;
use crate::way::ColumnGrid;
use crate::way::Segment;
use crate::way::StationGrid;

/// The base-2 logarithm of a brick's side in cells.
pub const BRICK_SHIFT: u32 = 6;
pub const BRICK: u32 = 1 << BRICK_SHIFT;
pub const BRICK_CELLS: u32 = BRICK * BRICK;

/// The zoom at which a clip level's bricks are counted: `BRICK_SHIFT` zooms coarser than its
/// cells.
pub const fn bricks_zoom(cell_zoom: Zoom) -> Zoom {
    cell_zoom - BRICK_SHIFT
}

/// Cell `[x, y]`'s index among a brick's cells, or a level's entries; this is the one place
/// it is worked out. It is Morton order, a quadtree's nodes laid out as the Z curve visits
/// them, so an entry's four children a level under it are four in a row and the cells around
/// a cell lie near it in memory, as a texture's tiling lays texels out for the GPU's 2D cache.
/// A cell at an even x and the one after it in x are two in a row.
pub const fn idx_of([x, y]: [u32; 2]) -> u32 {
    spread(x) | spread(y) << 1
}

/// The cell at index `i` (`idx_of`'s inverse).
pub const fn of_idx(i: u32) -> [u32; 2] {
    [gather(i), gather(i >> 1)]
}

/// `v`'s low sixteen bits spread to the even ones: a Morton code's half.
const fn spread(v: u32) -> u32 {
    let v = (v | v << 8) & 0x00ff_00ff;
    let v = (v | v << 4) & 0x0f0f_0f0f;
    let v = (v | v << 2) & 0x3333_3333;
    (v | v << 1) & 0x5555_5555
}

/// `v`'s even bits gathered to the low sixteen: `spread`'s inverse.
const fn gather(v: u32) -> u32 {
    let v = v & 0x5555_5555;
    let v = (v | v >> 1) & 0x3333_3333;
    let v = (v | v >> 2) & 0x0f0f_0f0f;
    let v = (v | v >> 4) & 0x00ff_00ff;
    (v | v >> 8) & 0x0000_ffff
}
const _: () = assert!(BRICK <= 1 << 16, "a brick's side spreads in sixteen bits");

/// A tile's ways are cut into bins, `WAYS_ACROSS` a side, each `1 << WAYS_SHIFT` cells. A
/// brick lays its lines out by bin (`LINES_ENTRIES_AT`), so a column reads only its bin's
/// segments, and its fill and the world's query make its spans from them.
pub const WAYS_ACROSS: u32 = 32;
pub const WAYS_SHIFT: u32 = (TILE_CELLS / WAYS_ACROSS).trailing_zeros();
pub const WAYS_BINS_ACROSS: u32 = BRICK >> WAYS_SHIFT;
pub const WAYS_BINS: u32 = WAYS_BINS_ACROSS * WAYS_BINS_ACROSS;

/// A brick's lines, laid out by the CPU while the brick is filled (bake/tile/terrain.rs
/// `brick_lines`), from which its fill makes its columns' spans and which it then gives back.
/// First comes its tile's header (`way::header`); then `WAYS_BINS + 1` words, each bin's first
/// entry and, after the last bin's, how many entries there are; then the entries, a word each,
/// each one of the brick's segments, a bin's in order; then the segments the bins reach, in
/// their lines' order, `way::SEGMENT_WORDS` each. No limit bounds how many: the words follow
/// the brick (`lines_words`).
pub const LINES_BINS_AT: u32 = way::HEADER_WORDS;
pub const LINES_ENTRIES_AT: u32 = LINES_BINS_AT + WAYS_BINS + 1;

/// The words of a brick's lines with `entries` entries over `segments` segments.
pub const fn lines_words(entries: u32, segments: u32) -> u32 {
    LINES_ENTRIES_AT + entries + segments * way::SEGMENT_WORDS as u32
}

/// One brick as its fill reads it, for the ways' one rule (`way::ColumnGrid`): its cells' zoom;
/// where it lies in its tile, in bricks; its first cell as the world names it, with X wrapped
/// there; what is known of the area at it (`area::Shard`), by which its columns are asked
/// whether they are the wall (`area::is_wall`); word `k` of its lines (`LINES_ENTRIES_AT`);
/// cell `i`'s own palette index as `columns` laid it (pxl-engine clipmap.rs `OWN_WORDS`); and
/// column `c`'s elevation from its tile's samples, with `c` counted from its first cell and the
/// ring of columns around it included. Its columns are its cells and that ring. A ring column
/// is read from the edge bin it borders, whose lines hold every line that reaches it
/// (`way::footprint`); a column past the ring has no segment.
pub trait Brick {
    fn cell_zoom(&self) -> Zoom;
    fn place(&self) -> [u32; 2];
    fn first(&self) -> [i32; 2];
    fn shard(&self) -> Shard;
    fn line(&self, k: u32) -> u32;
    /// Cell `i`'s own palette index as its fill laid it (pxl-engine clipmap.rs `OWN_WORDS`).
    fn own(&self, i: u32) -> u32;
    fn ele_at(&self, c: [i32; 2]) -> i32;
}

/// Whether column `c` is the brick's or its ring's.
fn in_brick(c: [i32; 2]) -> bool {
    c[0] >= -1 && c[1] >= -1 && c[0] <= BRICK as i32 && c[1] <= BRICK as i32
}

/// Column `c`'s cell index in the brick, none outside it.
fn cell_idx(c: [i32; 2]) -> u32 {
    if c[0] < 0 || c[1] < 0 || c[0] >= BRICK as i32 || c[1] >= BRICK as i32 {
        return BRICK_CELLS;
    }
    idx_of([c[0] as u32, c[1] as u32])
}

/// Column `c`'s bin word, which for a ring column is the edge bin it borders: its first entry
/// and its count; none past the ring.
fn bin_of(sq: &impl Brick, c: [i32; 2]) -> (u32, u32) {
    if !in_brick(c) {
        return (0, 0);
    }
    let (x, y) = (
        c[0].clamp(0, BRICK as i32 - 1) as u32,
        c[1].clamp(0, BRICK as i32 - 1) as u32,
    );
    let b = (y >> WAYS_SHIFT) * WAYS_BINS_ACROSS + (x >> WAYS_SHIFT);
    let first = sq.line(LINES_BINS_AT + b);
    (first, sq.line(LINES_BINS_AT + b + 1) - first)
}

/// Entry `e` of the brick's lines: the place of a segment among the brick's segments.
fn entry_of(sq: &impl Brick, e: u32) -> u32 {
    sq.line(LINES_ENTRIES_AT + e)
}

fn segment_at(sq: &impl Brick, at: u32) -> Segment {
    let entries = sq.line(LINES_BINS_AT + WAYS_BINS);
    let base = LINES_ENTRIES_AT + entries + at * way::SEGMENT_WORDS as u32;
    let mut w = [0; way::SEGMENT_WORDS];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(base + k as u32);
        k += 1;
    }
    Segment::of(w)
}

fn header_of(sq: &impl Brick) -> (f32, StationGrid) {
    let mut w = [0; way::HEADER_WORDS as usize];
    let mut k = 0;
    while k < w.len() {
        w[k] = sq.line(k as u32);
        k += 1;
    }
    way::of_header(w)
}

/// A brick's columns as its lines and its cells give them.
impl<Q: Brick> ColumnGrid for Q {
    fn cell_zoom(&self) -> Zoom {
        Brick::cell_zoom(self)
    }
    fn per_m(&self) -> f32 {
        header_of(self).0
    }
    fn cnt(&self, c: [i32; 2]) -> u32 {
        bin_of(self, c).1
    }
    fn segment(&self, c: [i32; 2], k: u32) -> Segment {
        segment_at(self, entry_of(self, bin_of(self, c).0 + k))
    }
    /// In eighths of its tile's cells: the brick's first cell, offset to the cell's middle.
    fn center(&self, c: [i32; 2]) -> [i32; 2] {
        let p = self.place();
        let eighths = way::POINT_SCALE as i32;
        let center = (lattice::CELL_CENTER * way::POINT_SCALE) as i32;
        [
            ((p[0] * BRICK) as i32 + c[0]) * eighths + center,
            ((p[1] * BRICK) as i32 + c[1]) * eighths + center,
        ]
    }
    /// The wall's one rule (`area::is_wall`), asked of the ring's columns too.
    fn is_walled(&self, c: [i32; 2]) -> bool {
        let (cell_zoom, first) = (Brick::cell_zoom(self), self.first());
        let b = [wrap_idx(cell_zoom, first[0] + c[0]), first[1] + c[1]];
        area::is_wall(self.shard(), cell_zoom, b)
    }
    fn stations(&self) -> StationGrid {
        header_of(self).1
    }
    /// Its own palette index as its fill laid it (pxl-engine clipmap.rs `OWN_WORDS`), the wall
    /// apart (`is_walled`).
    fn asset_id(&self, c: [i32; 2]) -> u32 {
        let i = cell_idx(c);
        if i == BRICK_CELLS {
            VOID
        } else {
            self.own(i)
        }
    }
    fn face(&self, c: [i32; 2]) -> i32 {
        if !in_brick(c) {
            return 0;
        }
        let cell_zoom = Brick::cell_zoom(self);
        ground::face_ele(cell_zoom, ground::cells(cell_zoom, self.ele_at(c) as f32))
    }
    /// Whether the entry is the one after the one before it, and its segment continues the one
    /// stored before it.
    fn joins(&self, c: [i32; 2], k: u32) -> bool {
        if k == 0 {
            return false;
        }
        let first = bin_of(self, c).0;
        let e = entry_of(self, first + k);
        e == entry_of(self, first + k - 1) + 1 && segment_at(self, e).continues == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::marker::{marker_cell_of, MARKER_SHOWN_FROM};
    use crate::space::coord::zoom::UNIT_ZOOM;
    use crate::space::hierarchy::tree::ancestor_of;
    use crate::space::layer::octant::Octant;

    /// A marker's cell lies in the window's brick drawing its grid's cells, so no cell's count
    /// is split between two bricks, wherever the brick is no finer than the marker.
    #[test]
    fn marker_cell_in_brick() {
        let markers = (0..400i32).map(|k| {
            Octant::new(
                UNIT_ZOOM,
                [(k * 7919) % 300, (k * 104_729) % 300, (k * 31) % 9],
            )
        });
        for at in markers {
            for cell_zoom in MARKER_SHOWN_FROM as Zoom..=at.zoom() + BRICK_SHIFT {
                let (z, c) = marker_cell_of(cell_zoom, at);
                let brick = bricks_zoom(cell_zoom);
                let [x, y, _] = ancestor_of(c, z - brick);
                let [ax, ay, _] = ancestor_of(at.at(), at.zoom() - brick);
                assert_eq!([x, y], [ax, ay], "{at:?} at {cell_zoom}");
            }
        }
    }
}
