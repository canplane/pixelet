// pxl-portable/ways.rs

//! gpu-portable: a tile's ways off the ground as a ray asks them — its
//! bridges' and tunnels' segments and the piers the bake stood, indexed by
//! the bins of the tile's cells they reach. The region is one lane of words,
//! stored as it is read (pxl-spec bake/ways.rs builds it), so the CPU reads
//! the tile's own and the GPU its frame's copy: a node asks its bin, and only
//! the few segments in it are swept. A way at grade and an approach's
//! embankment or cutting are in the ground's cells and looks, not here.
//!
//! A region, in words:
//!
//!   [segments, piers, slab depth, 0]
//!   [each bin's first entry, row by row, then the end]   BINS² + 1
//!   [entries, two a word, low first: a segment, or a pier past them]
//!   [segments, SEGMENT_WORDS each]
//!   [piers, PIER_WORDS each]

use crate::lattice::TILE_CELLS;
use crate::Zoom;

/// Cells a bin is across.
pub const BIN_CELLS: u32 = 16;
/// Bins a tile is across.
pub const BINS: u32 = TILE_CELLS / BIN_CELLS;

/// The header's words: segments held, piers held, a deck's slab depth in leaf
/// cells (the tile's latitude's), a pad.
pub const SEGMENTS: u32 = 0;
pub const PIERS: u32 = 1;
pub const SLAB: u32 = 2;
pub const HEADER: u32 = 4;
/// Each bin's first entry, the last word the entries' end.
pub const TABLE: u32 = HEADER;
pub const ENTRIES: u32 = TABLE + BINS * BINS + 1;

/// A segment: its two points in eighths of the tile's cells (X low, Y
/// high), its surface at each in leaf cells (low, high), and its line's word.
pub const SEGMENT_WORDS: u32 = 4;
/// A pier: its cell (X low, Y high) and its span in leaf cells, low to high.
pub const PIER_WORDS: u32 = 2;

/// A segment's line word: its width in eighths of a cell, its kind, its form,
/// whether it is its line's first or last (the line ends square there), and
/// a bore's clear height in leaf cells.
pub const WIDTH: u32 = 0;
pub const KIND: u32 = 8;
pub const FORM: u32 = 16;
pub const FIRST: u32 = 1 << 20;
pub const LAST: u32 = 1 << 21;
pub const CLEAR: u32 = 24;

/// A form's number, as pxl-spec `Form` is.
pub const BRIDGE: u32 = 0;
pub const TUNNEL: u32 = 1;

/// Points are in eighths of a cell.
pub const POINT_SCALE: f32 = 8.0;
/// The farthest past half its width a line is drawn, in its tile's cells.
pub const LINE_PAST: f32 = 1.0;

/// What a pier is built of (pxl-spec `ground::building`).
pub const PIER: u32 = crate::cube::ground::building as u32;

/// A region's words, wherever they live.
pub trait Lines {
    fn word(&self, i: u32) -> u32;
}

/// A region as a slice holds it.
pub struct Words<'a>(pub &'a [u32]);

impl Lines for Words<'_> {
    fn word(&self, i: u32) -> u32 {
        crate::at(self.0, i)
    }
}

fn lo16(w: u32) -> f32 {
    ((w as i32) << 16 >> 16) as f32
}

fn hi16(w: u32) -> f32 {
    ((w as i32) >> 16) as f32
}

/// The bin holding `v` on an axis, in the tile's cells, clamped to the tile.
pub fn bin(v: f32) -> u32 {
    ((v / BIN_CELLS as f32) as i32).clamp(0, BINS as i32 - 1) as u32
}

/// How far from its line a way of `width` cells covers a cell's centre
/// (pxl-spec bake/way.rs `reach`).
pub fn reach(width: f32) -> f32 {
    (width * 0.5).max(core::f32::consts::FRAC_1_SQRT_2)
}

/// Entry `e`: a segment's number, or a pier's past them.
fn entry(l: &impl Lines, e: u32) -> u32 {
    (l.word(ENTRIES + e / 2) >> (16 * (e % 2))) & 0xffff
}

/// Where segment `i`'s words start.
fn segment(l: &impl Lines, i: u32) -> u32 {
    let entries = l.word(TABLE + BINS * BINS);
    ENTRIES + entries.div_ceil(2) + SEGMENT_WORDS * i
}

/// Where pier `j`'s words start.
fn pier(l: &impl Lines, j: u32) -> u32 {
    segment(l, l.word(SEGMENTS)) + PIER_WORDS * j
}

/// Whether `[lo, hi)` fills the block `z`: half its height or more, so one
/// block where it is a leaf cell tall (the fold's half-cell rule).
fn fills(lo: i32, hi: i32, z: [i32; 2]) -> bool {
    let cover = hi.min(z[1]) - lo.max(z[0]);
    cover > 0 && 2 * cover >= z[1] - z[0]
}

/// Whether a way may stand in the square `sq` (the tile's cells, x0 y0 x1
/// y1, inside one bin) between heights `z` (leaf cells): a segment's
/// footprint reaches it where its slab or bore spans, or a pier stands in
/// it. A deck or a bore bounds itself so: a node it is not in is not split
/// for it.
pub fn may(l: &impl Lines, sq: [f32; 4], z: [i32; 2]) -> bool {
    let segments = l.word(SEGMENTS);
    if segments + l.word(PIERS) == 0 {
        return false;
    }
    let slab = l.word(SLAB) as i32;
    let b = bin((sq[1] + sq[3]) * 0.5) * BINS + bin((sq[0] + sq[2]) * 0.5);
    let mut e = l.word(TABLE + b);
    let end = l.word(TABLE + b + 1);
    while e < end {
        let id = entry(l, e);
        e += 1;
        if id >= segments {
            let p = pier(l, id - segments);
            let (cell, span) = (l.word(p), l.word(p + 1));
            let c = [(cell & 0xffff) as f32 + 0.5, (cell >> 16) as f32 + 0.5];
            if c[0] >= sq[0] && c[0] < sq[2] && c[1] >= sq[1] && c[1] < sq[3] && lo16(span) < z[1] as f32 && hi16(span) > z[0] as f32 {
                return true;
            }
            continue;
        }
        let s = segment(l, id);
        let (pa, pb, tops, line) = (l.word(s), l.word(s + 1), l.word(s + 2), l.word(s + 3));
        let half = (line >> WIDTH & 0xff) as f32 / POINT_SCALE * 0.5 + LINE_PAST;
        let a = [lo16(pa) / POINT_SCALE, hi16(pa) / POINT_SCALE];
        let c = [lo16(pb) / POINT_SCALE, hi16(pb) / POINT_SCALE];
        if a[0].max(c[0]) + half < sq[0] || a[0].min(c[0]) - half > sq[2] || a[1].max(c[1]) + half < sq[1] || a[1].min(c[1]) - half > sq[3] {
            continue;
        }
        let (ta, tb) = (lo16(tops), hi16(tops));
        let (lo, hi) = if line >> FORM & 0xf == BRIDGE {
            (ta.min(tb) - slab as f32, ta.max(tb))
        } else {
            (ta.min(tb), ta.max(tb) + (line >> CLEAR) as f32)
        };
        if lo < z[1] as f32 && hi >= z[0] as f32 {
            return true;
        }
    }
    false
}

/// A segment over a column: the surface it runs through across the column,
/// low and high, in leaf cells.
struct Over {
    low: i32,
    high: i32,
}

/// Whether segment `s` covers the column of side `side` about `at` (the
/// tile's cells), and the surface it runs through across it into `o`: the
/// nearest point of the segment within its reach, the line ending square
/// at its first and last points (pxl-spec separation.rs `crossing_segment`).
fn over(l: &impl Lines, s: u32, at: [f32; 2], side: f32, o: &mut Over) -> bool {
    let (pa, pb, tops, line) = (l.word(s), l.word(s + 1), l.word(s + 2), l.word(s + 3));
    let a = [lo16(pa) / POINT_SCALE, hi16(pa) / POINT_SCALE];
    let c = [lo16(pb) / POINT_SCALE, hi16(pb) / POINT_SCALE];
    let d = [c[0] - a[0], c[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let mut t = 0.0;
    if len2 > 0.0 {
        t = ((at[0] - a[0]) * d[0] + (at[1] - a[1]) * d[1]) / len2;
    }
    if (line & FIRST != 0 && t < 0.0) || (line & LAST != 0 && t > 1.0) {
        return false;
    }
    let t = t.clamp(0.0, 1.0);
    let (ex, ey) = (at[0] - (a[0] + d[0] * t), at[1] - (a[1] + d[1] * t));
    let r = reach((line >> WIDTH & 0xff) as f32 / POINT_SCALE);
    if ex * ex + ey * ey > r * r {
        return false;
    }
    // the column's square spans this much of the segment either side of `t`
    let mut across = 0.0;
    if len2 > 0.0 {
        across = side * 0.5 * (d[0].abs() + d[1].abs()) / len2;
    }
    let (ta, tb) = (lo16(tops), hi16(tops));
    let z0 = ta + (tb - ta) * (t - across).clamp(0.0, 1.0);
    let z1 = ta + (tb - ta) * (t + across).clamp(0.0, 1.0);
    o.low = libm::rintf(z0.min(z1)) as i32;
    o.high = libm::rintf(z0.max(z1)) as i32;
    true
}

/// What the ways write in the column of side `side` about `at` (the tile's
/// cells) between heights `z` (leaf cells): the highest slab or pier filling
/// it, a higher keeping what two share; else a bore's air, `air`, or the
/// bore's kind where `bore`; else `none`.
pub fn block(l: &impl Lines, at: [f32; 2], side: f32, z: [i32; 2], air: u32, none: u32, bore: bool) -> u32 {
    let segments = l.word(SEGMENTS);
    if segments + l.word(PIERS) == 0 {
        return none;
    }
    let slab = l.word(SLAB) as i32;
    let b = bin(at[1]) * BINS + bin(at[0]);
    let mut e = l.word(TABLE + b);
    let end = l.word(TABLE + b + 1);
    let (mut best_lo, mut best_hi) = (i32::MIN, i32::MIN);
    let (mut out, mut bored) = (none, none);
    let mut o = Over { low: 0, high: 0 };
    let h = side * 0.5;
    while e < end {
        let id = entry(l, e);
        e += 1;
        let (lo, hi, kind) = if id >= segments {
            let p = pier(l, id - segments);
            let (cell, span) = (l.word(p), l.word(p + 1));
            let c = [(cell & 0xffff) as f32 + 0.5, (cell >> 16) as f32 + 0.5];
            if !(c[0] >= at[0] - h && c[0] < at[0] + h && c[1] >= at[1] - h && c[1] < at[1] + h) {
                continue;
            }
            (lo16(span) as i32, hi16(span) as i32, PIER)
        } else {
            let s = segment(l, id);
            if !over(l, s, at, side, &mut o) {
                continue;
            }
            let line = l.word(s + 3);
            if line >> FORM & 0xf == TUNNEL {
                if fills(o.low, o.high + (line >> CLEAR) as i32, z) {
                    bored = if bore { line >> KIND & 0xff } else { air };
                }
                continue;
            }
            (o.low - slab, o.high, line >> KIND & 0xff)
        };
        // as the slabs lie lowest first, a higher one keeping what two share
        if fills(lo, hi, z) && (lo > best_lo || (lo == best_lo && hi > best_hi)) {
            (best_lo, best_hi) = (lo, hi);
            out = kind;
        }
    }
    if out != none {
        if bore {
            none
        } else {
            out
        }
    } else {
        bored
    }
}

/// The way's block about a bore's air, where the ground is: the kind of a
/// bore whose air is in a block across one of the six faces of the block
/// `z` of the column about `at`, else `none`. The caller asks it only where
/// the ground fills the block and the ways write nothing.
pub fn about(l: &impl Lines, at: [f32; 2], side: f32, z: [i32; 2], none: u32) -> u32 {
    let tall = z[1] - z[0];
    let mut k = 0;
    while k < 6 {
        let (dx, dy, dz) = match k {
            0 => (side, 0.0, 0),
            1 => (-side, 0.0, 0),
            2 => (0.0, side, 0),
            3 => (0.0, -side, 0),
            4 => (0.0, 0.0, tall),
            _ => (0.0, 0.0, -tall),
        };
        let kind = block(l, [at[0] + dx, at[1] + dy], side, [z[0] + dz, z[1] + dz], none, none, true);
        if kind != none {
            return kind;
        }
        k += 1;
    }
    none
}

/// A node's heights in leaf cells: the blocks it spans at `zoom`.
pub fn heights(zoom: Zoom, z: i32) -> [i32; 2] {
    let s = crate::LEAF_CELL - zoom;
    [z << s, (z + 1) << s]
}
