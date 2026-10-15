// pxl-portable/ways.rs

//! gpu-portable: the ways off the ground, as a block asks them — a bridge's
//! deck and its piers, a tunnel's bore and the way's block about it, an
//! approach's embankment or cutting — one rule for the CPU's grid and the
//! GPU's descent (pxl-spec bake/separation.rs keeps the lines, solved
//! offline). A block is asked of the lines over its column, each visit of a
//! line its own crossing: the run of its segments, one after another, that
//! are over the column. What is under half a block in any dimension at a
//! zoom is absent there, as the fold drops a line under half a cell wide: a
//! slab or a bore fills a block where it holds half its height or more, and
//! a pier, a leaf cell square, stands only in columns no more than two leaf
//! cells wide. Index loops, no `Option<bool>`, words for tags.

use crate::cube::{sentinel, Ground};
use crate::{Zoom, LEAF_CELL};

/// Points are eighths of the tile's cell.
pub const POINT_SCALE: f32 = 8.0;

/// The farthest past half its width a line is drawn, in its tile's cells.
pub const LINE_PAST: f32 = 1.0;

/// A deck's slab under its surface, in ground metres: Arnis's
/// `DECK_STRUCTURE_DEPTH`.
pub const DECK_THICKNESS_M: f32 = 2.0;

/// Clear height a deck keeps over a deck under it, in ground metres: Arnis's
/// `STACKED_DECK_HEADROOM`.
pub const STACKED_DECK_HEADROOM_M: f32 = 6.0;

// From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
// the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
// on a deck's line rather than place blocks. A bridge the source names no
// structure for is Arnis's beam bridge (`pier_interval`, `place_pier_bent`):
// a bent every 8 m, 12 m once its half width passes two cells, of one-cell
// columns.
pub const BEAM_PILLAR_INTERVAL_M: f32 = 8.0;
pub const WIDE_BEAM_PIER_INTERVAL_M: f32 = 12.0;

/// Arnis's `LOWER_DECK_GAP`, a block short of the stacked headroom: a deck at
/// least this far under a pier's blocks it.
const LOWER_DECK_GAP_M: f32 = STACKED_DECK_HEADROOM_M - 1.0;

/// Arnis's `place_pillar`: a deck this near the ground rests on it.
const PIER_MIN_RISE_M: f32 = 2.0;

/// What a pier is drawn as.
pub const PIER: u32 = Ground::building as u32;

/// A line's form, as its byte on the wire.
pub const BRIDGE: u32 = 0;
pub const TUNNEL: u32 = 1;
pub const APPROACH: u32 = 2;

/// What `block` answers where the ways write nothing there.
pub const NOTHING: u32 = u32::MAX;

/// What `block` answers for a bore's air.
pub const AIR: u32 = sentinel::void as u32;

/// Half a way's width, in whole cells.
pub fn half_cells(width: f32) -> i32 {
    libm::rintf(width / 2.0) as i32
}

/// How far from its line a way of `width` cells covers a cell's center: half
/// its width, and at least half a cell's diagonal so a diagonal way stays
/// side-connected.
pub fn reach(width: f32) -> f32 {
    (width * 0.5).max(core::f32::consts::FRAC_1_SQRT_2)
}

pub fn covers(width: f32, distance: f32) -> bool {
    distance <= reach(width)
}

/// How far from its line a way reaches anything drawn, in its tile's cells.
pub fn footprint(width: f32) -> f32 {
    width / 2.0 + LINE_PAST
}

/// A deck's slab in units of height, at least one.
pub fn slab_depth(u_per_m: f32) -> i32 {
    (libm::rintf(DECK_THICKNESS_M * u_per_m) as i32).max(1)
}

/// One segment of a line, in its tile's cells: from `a` to `b`, its road
/// surface `tops` there in leaf cells, its width in cells, the look it shows,
/// its form, its tunnel's clear height in leaf cells, and whether it is its
/// line's first or last, where the line ends square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub tops: [f32; 2],
    pub width: f32,
    pub kind: u32,
    pub form: u32,
    pub clear: u32,
    pub first: u32,
    pub last: u32,
}

/// A segment's words: `a` and `b` each two `i16` eighths of a cell, the tops
/// two `i16`, then width (eighths), kind, form with first and last over it,
/// and clear, a byte each.
pub const SEGMENT_WORDS: usize = 4;

/// A segment's form with whether it is its line's first and last, a byte.
pub fn flags(form: u32, first: bool, last: bool) -> u8 {
    ((form & 3) | u32::from(first) << 2 | u32::from(last) << 3) as u8
}

/// A word's two `i16`, sign-extended in words, as a GPU has no `i16`.
fn i16_pair(w: u32) -> [f32; 2] {
    [((w << 16) as i32 >> 16) as f32, (w as i32 >> 16) as f32]
}

fn pair_word(lo: i16, hi: i16) -> u32 {
    u32::from(lo as u16) | u32::from(hi as u16) << 16
}

impl Segment {
    pub fn of(w: [u32; SEGMENT_WORDS]) -> Segment {
        let [a, b] = [i16_pair(w[0]), i16_pair(w[1])];
        let byte = |k: u32| (w[3] >> (8 * k)) & 0xff;
        Segment {
            a: [a[0] / POINT_SCALE, a[1] / POINT_SCALE],
            b: [b[0] / POINT_SCALE, b[1] / POINT_SCALE],
            tops: i16_pair(w[2]),
            width: byte(0) as f32 / POINT_SCALE,
            kind: byte(1),
            form: byte(2) & 3,
            first: (byte(2) >> 2) & 1,
            last: (byte(2) >> 3) & 1,
            clear: byte(3),
        }
    }

    /// Its words, from the wire's integers: its two points in eighths, tops in
    /// leaf cells, and its bytes — width in eighths, kind, `flags` (`flags`),
    /// clear.
    pub fn words([a, b]: [[i16; 2]; 2], tops: [i16; 2], [width, kind, flags, clear]: [u8; 4]) -> [u32; SEGMENT_WORDS] {
        [
            pair_word(a[0], a[1]),
            pair_word(b[0], b[1]),
            pair_word(tops[0], tops[1]),
            u32::from(width) | u32::from(kind) << 8 | u32::from(flags) << 16 | u32::from(clear) << 24,
        ]
    }

    /// Whether its footprint reaches the box `[lo, hi]` of the tile's cells.
    pub fn reaches(&self, lo: [f32; 2], hi: [f32; 2]) -> bool {
        let half = footprint(self.width);
        self.a[0].max(self.b[0]) + half >= lo[0] && self.a[0].min(self.b[0]) - half <= hi[0] && self.a[1].max(self.b[1]) + half >= lo[1] && self.a[1].min(self.b[1]) - half <= hi[1]
    }
}

/// Where a tile's piers may stand: the bents' lattice along each axis, a
/// step of `step[w]` cells for the narrow (`w` 0) and the wide (1) beam, and
/// where the tile's corner lies inside a step, `phase[w][axis]`, so a station
/// is the world's and not the tile's; and the leaf cells a tile's cell
/// spans a side, which a pier's section is measured in. Made once a tile in
/// doubles (pxl-spec procedural.rs `Frame::stations`), as a corner is far
/// past where a float holds a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stations {
    pub step: [f32; 2],
    pub phase: [[f32; 2]; 2],
    pub leaf: f32,
}

/// A bent's spacing for the narrow beam (`w` 0) and the wide (1), metres.
pub const INTERVAL_M: [f32; 2] = [BEAM_PILLAR_INTERVAL_M, WIDE_BEAM_PIER_INTERVAL_M];

/// A bent's columns across the deck, from its line: the middle, both edges,
/// or both and the middle, by its half width in cells; `count` of them.
fn bent(half: i32) -> ([i32; 3], u32) {
    match half {
        ..=2 => ([0, 0, 0], 1),
        3..=4 => ([-1, 1, 0], 2),
        _ => ([-1, 0, 1], 3),
    }
}

/// Whether the column of `side` about `at` holds the pier at the station
/// nearest `near` on `[a, a + d)`, in the tile's cells; half-open, so a
/// joint's station is one segment's. A pier is a leaf cell square, its bent
/// laid out in leaf cells: a column more than two leaf cells wide holds none,
/// as it is under half of it; one of two holds it where one of its four leaf
/// cells does.
pub fn pier_covers(st: &Stations, width: f32, a: [f32; 2], d: [f32; 2], near: [f32; 2], at: [f32; 2], side: f32) -> bool {
    let k = if d[0].abs() >= d[1].abs() { 0 } else { 1 };
    let leaves = side * st.leaf;
    if d[k] == 0.0 || leaves > 2.0 {
        return false;
    }
    let half = half_cells(width * st.leaf);
    let w = if half <= 2 { 0 } else { 1 };
    let (step, phase) = (st.step[w], st.phase[w][k]);
    let station = libm::rintf((phase + near[k]) / step) * step - phase;
    let t = (station - a[k]) / d[k];
    if !(0.0..1.0).contains(&t) {
        return false;
    }
    let len = libm::hypotf(d[0], d[1]);
    let dir = [d[0] / len, d[1] / len];
    let foot = [a[0] + d[0] * t, a[1] + d[1] * t];
    // A leaf cell, and the centers of the leaf cells the column holds: its
    // own where it is one or finer, else its four.
    let leaf = 1.0 / st.leaf;
    let (off, m) = if leaves >= 2.0 { (leaf / 2.0, 4) } else { (0.0, 1) };
    let (sides, n) = bent(half);
    let mut covered = false;
    let mut j = 0;
    while j < m {
        let p = [at[0] + if j & 1 == 0 { -off } else { off }, at[1] + if j & 2 == 0 { -off } else { off }];
        let rel = [p[0] - foot[0], p[1] - foot[1]];
        let along = (rel[0] * dir[0] + rel[1] * dir[1]).abs();
        let across = rel[0] * dir[1] - rel[1] * dir[0];
        // an edge column stands a leaf cell in from the edge
        let mut i = 0;
        while i < n as usize {
            covered |= along <= leaf / 2.0 && (across - (sides[i] * (half - 1).max(0)) as f32 * leaf).abs() <= leaf / 2.0;
            i += 1;
        }
        j += 1;
    }
    covered
}

/// A segment over one column: the lowest and highest its road surface runs
/// across the column, in leaf cells, and whether its pier's section covers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub low: f32,
    pub top: f32,
    pub kind: u32,
    pub form: u32,
    pub pier: u32,
    pub clear: u32,
}

/// What segment `s` is over the column of `side` cells about `at`, if its way
/// covers `at`: the surface it runs through across the column, and whether
/// `at` is inside the section of a pier at a station of it. A line ends square
/// where it begins and ends, as the ground cuts a way; its joints stay round.
pub fn crossing(s: &Segment, at: [f32; 2], side: f32, st: &Stations) -> Option<Crossing> {
    let (a, b) = (s.a, s.b);
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 { ((at[0] - a[0]) * d[0] + (at[1] - a[1]) * d[1]) / len2 } else { 0.0 };
    if (s.first == 1 && t < 0.0) || (s.last == 1 && t > 1.0) {
        return None;
    }
    let t = t.clamp(0.0, 1.0);
    let near = [a[0] + d[0] * t, a[1] + d[1] * t];
    if !covers(s.width, libm::hypotf(at[0] - near[0], at[1] - near[1])) {
        return None;
    }
    // the column's square spans this much of the segment either side of `t`
    let reach = if len2 > 0.0 { side / 2.0 * (d[0].abs() + d[1].abs()) / len2 } else { 0.0 };
    let z = |t: f32| s.tops[0] + (s.tops[1] - s.tops[0]) * t.clamp(0.0, 1.0);
    let (z0, z1) = (z(t - reach), z(t + reach));
    Some(Crossing {
        low: z0.min(z1),
        top: z0.max(z1),
        kind: s.kind,
        form: s.form,
        pier: u32::from(s.form != TUNNEL && pier_covers(st, s.width, a, d, near, at, side)),
        clear: s.clear,
    })
}

/// What a crossing occupies of its column, in leaf cells, each `[lo, hi)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Occupy {
    /// The way on the ground; the column's top cell is its surface.
    Grade,
    /// A slab under the surface `span`, and a pier from the ground to it
    /// where its section covers the column (`pier`, empty where none).
    Over,
    /// Air from the surface up `span`; the cell under it is the way's.
    Under,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Occupies {
    pub what: Occupy,
    pub span: [i32; 2],
    pub pier: [i32; 2],
}

/// What `c` occupies over ground `ground` leaf cells high, `u_per_m` units of
/// height a ground metre: all the surface sweeps across the column, so a slab
/// or a bore steps down to the next column's without a gap however steep.
pub fn occupies(c: &Crossing, ground: i32, u_per_m: f32) -> Occupies {
    let (low, top) = (libm::rintf(c.low) as i32, libm::rintf(c.top) as i32);
    let none = [0, 0];
    match c.form {
        BRIDGE => {
            let lo = low - slab_depth(u_per_m);
            let pier = if c.pier == 1 && lo > ground { [ground, lo] } else { none };
            Occupies {
                what: Occupy::Over,
                span: [lo, top],
                pier,
            }
        }
        TUNNEL => Occupies {
            what: Occupy::Under,
            span: [low, top + c.clear as i32],
            pier: none,
        },
        // a way on the ground moves the ground with it: filled up to it, cut down to it
        _ if top > ground => Occupies {
            what: Occupy::Over,
            span: [ground, top],
            pier: none,
        },
        _ if top < ground => Occupies {
            what: Occupy::Under,
            span: [low, ground],
            pier: none,
        },
        _ => Occupies {
            what: Occupy::Grade,
            span: none,
            pier: none,
        },
    }
}

/// The columns a grid of one zoom reads its ways off: each one's segments, in
/// its own tile's cells, with its center there and the tile's stations, and
/// its ground — its look as read, and its face's height in leaf cells. A
/// column the grid does not hold has no segment.
pub trait Columns {
    /// The cells' zoom, and units of height a ground metre over the grid.
    fn cell(&self) -> Zoom;
    fn per_m(&self) -> f32;
    /// How many segments may be over column `c`, and the `k`-th of them.
    fn count(&self, c: [i32; 2]) -> u32;
    fn segment(&self, c: [i32; 2], k: u32) -> Segment;
    /// The column's center and side in its tile's cells.
    fn center(&self, c: [i32; 2]) -> [f32; 2];
    fn side(&self, c: [i32; 2]) -> f32;
    fn stations(&self, c: [i32; 2]) -> Stations;
    fn look(&self, c: [i32; 2]) -> u32;
    fn face(&self, c: [i32; 2]) -> i32;
    /// Whether the `k`-th segment over column `c` is the next of its line
    /// after the one before it there, so the two are one visit.
    fn joins(&self, c: [i32; 2], k: u32) -> bool;
}

/// The `k`-th segment's crossing of column `c`.
fn crossing_at<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> Option<Crossing> {
    crossing(&cols.segment(c, k), cols.center(c), cols.side(c), &cols.stations(c))
}

/// The visit of a line over column `c` from its `k`-th segment there: the
/// run of segments from it, each the next of its line and each over the
/// column, as one crossing — all the surface they sweep across the column,
/// and a pier where any one's station is — and where the next visit starts.
/// None where the `k`-th is not over it.
fn visit<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> (Option<Crossing>, u32) {
    let Some(mut x) = crossing_at(cols, c, k) else { return (None, k + 1) };
    let mut j = k + 1;
    while j < cols.count(c) && cols.joins(c, j) {
        let Some(y) = crossing_at(cols, c, j) else { break };
        (x.low, x.top, x.pier) = (x.low.min(y.low), x.top.max(y.top), x.pier | y.pier);
        j += 1;
    }
    (Some(x), j)
}

/// Whether a crossing of column `c` writes there: a look that grounds, on a
/// column that is not the bedrock, as bedrock wins over a way as it does
/// over the ground's face.
fn writes<C: Columns>(cols: &C, c: [i32; 2], x: &Crossing) -> bool {
    Ground::grounds(x.kind) && cols.look(c) != sentinel::unknown as u32
}

/// Whether the leaf cells `[lo, hi)` fill block `z` of the grid's zoom: half
/// its height or more, so a run thinner than half a block fills none.
pub fn fills(cell: Zoom, [lo, hi]: [i32; 2], z: i32) -> bool {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        let leaf = z >> d;
        return lo <= leaf && leaf < hi;
    }
    let s = 1 << -d;
    2 * (hi.min((z + 1) * s) - lo.max(z * s)) >= s
}

/// Whether a run is empty, as a pier none stands for is.
fn empty(v: [i32; 2]) -> bool {
    v[0] >= v[1]
}

/// Whether run `v` sorts at or after `best`, by its low end, then its high.
fn at_or_after(v: [i32; 2], best: [i32; 2]) -> bool {
    v[0] > best[0] || (v[0] == best[0] && v[1] >= best[1])
}

/// None under a deck `PIER_MIN_RISE_M` or less over the ground, on a way at
/// grade or over a deck `LOWER_DECK_GAP_M` or more under its own (Arnis
/// `place_pillar`, `support_blocked`), none where the terrain gives no surface
/// to stand on; water stops none. Empty where none.
fn pier_stands<C: Columns>(cols: &C, c: [i32; 2], pier: [i32; 2], ground: i32, u_per_m: f32) -> [i32; 2] {
    let none = [0, 0];
    match Ground::of(cols.look(c) as u8) {
        Some(g) if !g.transport() => {}
        _ => return none,
    }
    let surface = pier[1] + slab_depth(u_per_m);
    if ((surface - pier[0]) as f32) <= PIER_MIN_RISE_M * u_per_m {
        return none;
    }
    let gap = libm::rintf(LOWER_DECK_GAP_M * u_per_m) as i32;
    let mut k = 0;
    while k < cols.count(c) {
        let (x, next) = visit(cols, c, k);
        k = next;
        if let Some(x) = x {
            let o = occupies(&x, ground, u_per_m);
            if o.what == Occupy::Over && o.span[1] <= surface - gap {
                return none;
            }
        }
    }
    pier
}

/// The way of the first bore of column `c`, in its segments' order, whose
/// air holds block `z[0]` or `z[1]`; `NOTHING` where none does.
fn bore<C: Columns>(cols: &C, c: [i32; 2], z: [i32; 2]) -> u32 {
    let (cell, face, per_m) = (cols.cell(), cols.face(c), cols.per_m());
    let mut k = 0;
    while k < cols.count(c) {
        let (x, next) = visit(cols, c, k);
        k = next;
        if let Some(x) = x.filter(|x| writes(cols, c, x)) {
            let o = occupies(&x, face, per_m);
            if o.what == Occupy::Under && (fills(cell, o.span, z[0]) || fills(cell, o.span, z[1])) {
                return x.kind;
            }
        }
    }
    NOTHING
}

/// The columns about a block whose bores it may border, in the order of the
/// grid's cells, row by row: the row under it, the cell before it, its own
/// (the blocks under and over it), the cell after it, the row over it.
const NEAR: [[i32; 2]; 5] = [[0, -1], [-1, 0], [0, 0], [1, 0], [0, 1]];

/// What the ways write in block `b` of the grid's zoom: a deck's slab or its
/// pier, the highest of those over it keeping what two share; else a bore's
/// air (`AIR`); else, where the ground is (`ground`), the way's block about a
/// bore beside it, so a cave written under it never opens into it; else
/// `NOTHING`, the ground's block standing.
pub fn block<C: Columns>(cols: &C, b: [i32; 3], ground: bool) -> u32 {
    let (c, cell) = ([b[0], b[1]], cols.cell());
    let (face, per_m) = (cols.face(c), cols.per_m());
    // The highest slab or pier over `b`, by its run's ends in leaf cells,
    // then its look.
    let (mut best, mut look, mut air) = ([i32::MIN, i32::MIN], NOTHING, false);
    let mut k = 0;
    while k < cols.count(c) {
        let (x, next) = visit(cols, c, k);
        k = next;
        if let Some(x) = x.filter(|x| writes(cols, c, x)) {
            let o = occupies(&x, face, per_m);
            match o.what {
                Occupy::Under => air |= fills(cell, o.span, b[2]),
                Occupy::Over => {
                    if fills(cell, o.span, b[2]) && at_or_after(o.span, best) {
                        (best, look) = (o.span, x.kind);
                    }
                    if !empty(o.pier) {
                        let pier = pier_stands(cols, c, o.pier, face, per_m);
                        if !empty(pier) && fills(cell, pier, b[2]) && at_or_after(pier, best) {
                            (best, look) = (pier, PIER);
                        }
                    }
                }
                // An approach's look, laid into the cells by the bake (pxl-spec bake/way.rs `lay`).
                Occupy::Grade => {}
            }
        }
    }
    if look != NOTHING {
        return look;
    }
    if air {
        return AIR;
    }
    if !ground {
        return NOTHING;
    }
    let mut d = 0;
    while d < NEAR.len() {
        let [dx, dy] = NEAR[d];
        let z = if dx == 0 && dy == 0 { [b[2] - 1, b[2] + 1] } else { [b[2], b[2]] };
        let way = bore(cols, [b[0] + dx, b[1] + dy], z);
        if way != NOTHING {
            return way;
        }
        d += 1;
    }
    NOTHING
}
