// pxl-portable/ways.rs

//! gpu-portable: the ways off the ground, as a block asks them — a bridge's
//! deck and its piers, a tunnel's bore and the way's block about it, an
//! approach's embankment or cutting — one rule for a square's fill, which
//! writes each column's runs once, and the descent on both sides, which
//! reads them (pxl-spec bake/separation.rs keeps the lines, solved
//! offline). A block is asked of the lines over its column, each visit of a
//! line its own crossing: the run of its segments, one after another, that
//! are over the column. A column's visits make its spans once (`column`),
//! what a block of it is is read off them (`at`), and ground beside the air
//! they leave takes that way's look
//! (`block`'s surround). What is under half a block in any dimension at a
//! zoom is absent there, as the fold drops a line under half a cell wide: a
//! slab or a bore fills a block where it holds half its height or more, and
//! a pier, a leaf cell square, stands only in columns no more than two leaf
//! cells wide. Index loops, no `Option` (a crossing none is is `Crossing::NONE`), words
//! for tags, nothing a GPU lacks (`hypot`).

use crate::cube::{sentinel, Ground};
use crate::lattice::GROUND;
use crate::{cells_of_m, Zoom, LEAF_CELL, LEAF_ZOOM};

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

/// A vector's length, as a GPU takes one: no `hypot` there.
fn length(v: [f32; 2]) -> f32 {
    libm::sqrtf(v[0] * v[0] + v[1] * v[1])
}

/// Whether a way of `width` cells is none at those cells: its width rounds
/// to no cell (half to even, so half a cell is none), what a fold discards.
pub fn vanishes(width: f32) -> bool {
    libm::rintf(width) == 0.0
}

/// The coarsest zoom a line's point is held at, off its weight: its
/// effective area on its whole line (Visvalingam–Whyatt, as TopoJSON's
/// presimplify and mapshaper weigh a point), in leaf cells². A zoom holds
/// the points that weigh a cell of its own or more, so what a zoom keeps is
/// a function of each point alone, the same in every tile holding it; a
/// line's ends weigh without bound and are held at every zoom.
pub fn coarsest(weight: f32) -> Zoom {
    let mut z = LEAF_ZOOM;
    while z > GROUND.top() && weight >= (1u32 << (2 * (LEAF_ZOOM - z + 1))) as f32 {
        z -= 1;
    }
    z
}

/// Whether a point held from `coarsest` on is held at `zoom`.
pub fn held_at(coarsest: Zoom, zoom: Zoom) -> bool {
    zoom >= coarsest
}

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

/// How far from its line a way reaches anything drawn, in its tile's cells:
/// its reach (`reach`), and a cell past it, so a tile's lines and a square's
/// hold every line whose air or slab can stand in the ring of cells about
/// them, a thin way's included.
pub fn footprint(width: f32) -> f32 {
    reach(width) + LINE_PAST
}

/// A deck's slab in leaf cells, at least one, `per_m` leaf cells a metre.
pub fn slab_depth(per_m: f32) -> i32 {
    cells_of_m(DECK_THICKNESS_M, per_m).max(1)
}

/// One segment of a line, in its tile's cells: from `a` to `b`, its road
/// surface `tops` there in leaf cells, its width in cells, the look it shows,
/// its form, its tunnel's clear height in leaf cells, whether it is its
/// line's first or last, where the line ends square, and whether the one
/// stored before it is its line's one before it.
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
    pub continues: u32,
}

/// A segment's words: `a` and `b` each two `i16` eighths of a cell, the tops
/// two `i16`, then width (eighths), kind, form with first, last and
/// continues over it, and clear, a byte each.
pub const SEGMENT_WORDS: usize = 4;

/// A segment's form with whether it is its line's first and last and
/// continues the one stored before it, a byte.
pub fn flags(form: u32, first: bool, last: bool, continues: bool) -> u8 {
    ((form & 3) | u32::from(first) << 2 | u32::from(last) << 3 | u32::from(continues) << 4) as u8
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
            continues: (byte(2) >> 4) & 1,
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

/// What a square's lines open with (pxl-portable window.rs `LINES_*`): its
/// tile's leaf cells a ground metre (pxl-spec `Frame::per_m`), then its
/// stations, step, phase and leaf, each a float's bits.
pub const HEADER_WORDS: u32 = 8;

pub fn header(per_m: f32, st: &Stations) -> [u32; HEADER_WORDS as usize] {
    let [[p0, p1], [p2, p3]] = st.phase;
    [
        per_m.to_bits(),
        st.step[0].to_bits(),
        st.step[1].to_bits(),
        p0.to_bits(),
        p1.to_bits(),
        p2.to_bits(),
        p3.to_bits(),
        st.leaf.to_bits(),
    ]
}

pub fn of_header(w: [u32; HEADER_WORDS as usize]) -> (f32, Stations) {
    let st = Stations {
        step: [f32::from_bits(w[1]), f32::from_bits(w[2])],
        phase: [[f32::from_bits(w[3]), f32::from_bits(w[4])], [f32::from_bits(w[5]), f32::from_bits(w[6])]],
        leaf: f32::from_bits(w[7]),
    };
    (f32::from_bits(w[0]), st)
}

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
    let len = length(d);
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

impl Crossing {
    /// No crossing: of kind `NOTHING`.
    pub const NONE: Crossing = Crossing {
        low: 0.0,
        top: 0.0,
        kind: NOTHING,
        form: 0,
        pier: 0,
        clear: 0,
    };

    /// Whether it is one (`NONE` is not).
    pub fn is(&self) -> bool {
        self.kind != NOTHING
    }
}

/// The road surface of `s` at the share `t` of it, clamped to its ends.
fn surface(s: &Segment, t: f32) -> f32 {
    s.tops[0] + (s.tops[1] - s.tops[0]) * t.clamp(0.0, 1.0)
}

/// Where `at` lies along the segment from `a` to `b` of a way `width` cells
/// wide, as a share of it clamped to it, and whether the way covers `at`
/// there: a line ends square where it begins (`ends[0]`) and ends
/// (`ends[1]`), as the ground cuts a way; its joints stay round. What the
/// ground's cells and the ways' columns are covered by alike.
pub fn on_way(a: [f32; 2], b: [f32; 2], width: f32, ends: [bool; 2], at: [f32; 2]) -> (f32, bool) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 { ((at[0] - a[0]) * d[0] + (at[1] - a[1]) * d[1]) / len2 } else { 0.0 };
    let cut = (ends[0] && t < 0.0) || (ends[1] && t > 1.0);
    let t = t.clamp(0.0, 1.0);
    let near = [a[0] + d[0] * t, a[1] + d[1] * t];
    (t, !cut && covers(width, length([at[0] - near[0], at[1] - near[1]])))
}

/// What segment `s` is over the column of `side` cells about `at`, `NONE`
/// unless its way covers `at` (`on_way`): the surface it runs through
/// across the column, and whether the column holds the section of a pier at
/// a station of it.
pub fn crossing(s: &Segment, at: [f32; 2], side: f32, st: &Stations) -> Crossing {
    let (a, b) = (s.a, s.b);
    let (t, on) = on_way(a, b, s.width, [s.first == 1, s.last == 1], at);
    if !on {
        return Crossing::NONE;
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let near = [a[0] + d[0] * t, a[1] + d[1] * t];
    // the column's square spans this much of the segment either side of `t`
    let reach = if len2 > 0.0 { side / 2.0 * (d[0].abs() + d[1].abs()) / len2 } else { 0.0 };
    let (z0, z1) = (surface(s, t - reach), surface(s, t + reach));
    Crossing {
        low: z0.min(z1),
        top: z0.max(z1),
        kind: s.kind,
        form: s.form,
        pier: u32::from(s.form != TUNNEL && pier_covers(st, s.width, a, d, near, at, side)),
        clear: s.clear,
    }
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

/// What `c` occupies over ground `ground` leaf cells high, `per_m` leaf
/// cells a ground metre: all the surface sweeps across the column, so a slab
/// or a bore steps down to the next column's without a gap however steep.
pub fn occupies(c: &Crossing, ground: i32, per_m: f32) -> Occupies {
    let (low, top) = (libm::rintf(c.low) as i32, libm::rintf(c.top) as i32);
    let none = [0, 0];
    match c.form {
        BRIDGE => {
            let lo = low - slab_depth(per_m);
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

/// The columns a square's fill reads its ways off (`window::Square`): each
/// one's segments, in its own tile's cells, with its center there and the
/// tile's stations, and its ground — its look as read, and its face's height
/// in leaf cells. A column the square does not hold has no segment.
pub trait Columns {
    /// The cells' zoom, and leaf cells a ground metre over the grid.
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
fn crossing_at<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> Crossing {
    crossing(&cols.segment(c, k), cols.center(c), cols.side(c), &cols.stations(c))
}

/// The visit of a line over column `c` from its `k`-th segment there: the
/// run of segments from it, each the next of its line and each over the
/// column, as one crossing — all the surface they sweep across the column,
/// and a pier where any one's station is — and where the next visit starts.
/// `NONE` where the `k`-th is not over it.
fn visit<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> (Crossing, u32) {
    let mut x = crossing_at(cols, c, k);
    if !x.is() {
        return (x, k + 1);
    }
    let mut j = k + 1;
    while j < cols.count(c) && cols.joins(c, j) {
        let y = crossing_at(cols, c, j);
        if !y.is() {
            break;
        }
        (x.low, x.top, x.pier) = (x.low.min(y.low), x.top.max(y.top), x.pier | y.pier);
        j += 1;
    }
    (x, j)
}

/// Whether the leaf cells `[lo, hi)` fill block `z` of the grid's zoom: more
/// than half its height, as its share of the block rounds (half to even), so
/// a run of half a block or thinner fills none.
pub fn fills(cell: Zoom, [lo, hi]: [i32; 2], z: i32) -> bool {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        let leaf = z >> d;
        return lo <= leaf && leaf < hi;
    }
    let s = 1 << -d;
    2 * (hi.min((z + 1) * s) - lo.max(z * s)) > s
}

/// The blocks of the grid's zoom the leaf cells `[lo, hi)` fill (`fills`),
/// `[first, last]`, first past last where none: every block between the
/// first and the last they reach is filled whole, so only those two can
/// fail, and a block finer than a leaf fills as its leaf does.
pub fn filled(cell: Zoom, [lo, hi]: [i32; 2]) -> [i32; 2] {
    let d = LEAF_CELL as i32 - cell as i32;
    if lo >= hi {
        return [1, 0];
    }
    if d <= 0 {
        return [lo << -d, (hi << -d) - 1];
    }
    let (first, last) = (lo >> d, (hi - 1) >> d);
    [if fills(cell, [lo, hi], first) { first } else { first + 1 }, if fills(cell, [lo, hi], last) { last } else { last - 1 }]
}

/// Whether the leaf cells `[lo, hi)` fill any block of the grid's zoom.
pub fn fills_any(cell: Zoom, run: [i32; 2]) -> bool {
    let [first, last] = filled(cell, run);
    first <= last
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
fn pier_stands<C: Columns>(cols: &C, c: [i32; 2], pier: [i32; 2], ground: i32, per_m: f32) -> [i32; 2] {
    let (none, look) = ([0, 0], cols.look(c));
    if !Ground::grounds(look) || Ground::transports(look) {
        return none;
    }
    let surface = pier[1] + slab_depth(per_m);
    if surface - pier[0] <= cells_of_m(PIER_MIN_RISE_M, per_m) {
        return none;
    }
    let gap = cells_of_m(LOWER_DECK_GAP_M, per_m);
    let mut k = 0;
    while k < cols.count(c) {
        let (x, next) = visit(cols, c, k);
        k = next;
        if x.is() {
            let o = occupies(&x, ground, per_m);
            if o.what == Occupy::Over && o.span[1] <= surface - gap {
                return none;
            }
        }
    }
    pier
}

/// What a run's value marks as a bore's air, over the look of its way.
pub const AIR_OF: u32 = 0x80;

/// What one visit writes in its column, in leaf cells: a deck's slab or an
/// embankment (its way's look), or a bore's or a cutting's air (`AIR_OF`
/// over its way's look), over `run`; and a pier (`PIER`) over `pier`,
/// empty where none stands. `NOTHING` where it writes none.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Writes {
    run: [i32; 2],
    value: u32,
    pier: [i32; 2],
}

impl Writes {
    const NONE: Writes = Writes {
        run: [0, 0],
        value: NOTHING,
        pier: [0, 0],
    };
}

/// What the visit from the `k`-th segment over column `c` writes, and where
/// the next visit starts: a visit of a look that grounds, as what it
/// occupies over the column's face, its pier where one stands.
fn writes<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> (Writes, u32) {
    let (x, next) = visit(cols, c, k);
    if !x.is() || !Ground::grounds(x.kind) {
        return (Writes::NONE, next);
    }
    let (face, per_m) = (cols.face(c), cols.per_m());
    let o = occupies(&x, face, per_m);
    let w = match o.what {
        Occupy::Under => Writes {
            run: o.span,
            value: AIR_OF | x.kind,
            pier: [0, 0],
        },
        Occupy::Over => Writes {
            run: o.span,
            value: x.kind,
            pier: if empty(o.pier) { o.pier } else { pier_stands(cols, c, o.pier, face, per_m) },
        },
        // An approach's look, laid into the cells by the bake (pxl-spec bake/way.rs `lay`).
        Occupy::Grade => Writes::NONE,
    };
    (w, next)
}

/// The most spans one column's visits make (`column`): the most of the
/// format-12 set (2026-10-17), measured with every tile at every zoom (z14's
/// 10 at 01033210230311; z15's most 9, none past z11); a tile past it fails
/// its bake (pxl-spec bake/tile/terrain.rs `runs_fit`).
pub const SPANS: usize = 10;

/// A column's spans, in leaf cells, in the order its visits come: each a
/// deck's slab or an embankment (its way's look), a pier (`PIER`), or a
/// bore's or a cutting's air (`AIR_OF` over its way's look); `n` of them.
/// The sweep's working set: its visits found once, made into the column's
/// runs (`runs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spans {
    pub run: [[i32; 2]; SPANS],
    pub value: [u32; SPANS],
    pub n: u32,
}

impl Spans {
    pub const NONE: Spans = Spans {
        run: [[0, 0]; SPANS],
        value: [0; SPANS],
        n: 0,
    };

    /// Span `v` over `run`, where there is room; false where there is none.
    fn push(&mut self, run: [i32; 2], v: u32) -> bool {
        if self.n as usize >= SPANS {
            return false;
        }
        self.run[self.n as usize] = run;
        self.value[self.n as usize] = v;
        self.n += 1;
        true
    }
}

/// Column `c`'s spans: what each visit of a line writes there that fills a
/// block of the grid's zoom (`fills_any`), as `at` reads no other — none in
/// a column of the bedrock, as bedrock wins over a way as it does over the
/// ground's face — and whether they all fit `SPANS`.
pub fn column<C: Columns>(cols: &C, c: [i32; 2]) -> (Spans, bool) {
    let (mut spans, mut fits) = (Spans::NONE, true);
    if cols.look(c) == crate::ground::UNKNOWN {
        return (spans, fits);
    }
    let mut k = 0;
    while k < cols.count(c) {
        let (w, next) = writes(cols, c, k);
        k = next;
        let cell = cols.cell();
        if w.value != NOTHING && fills_any(cell, w.run) {
            fits &= spans.push(w.run, w.value);
        }
        if w.value != NOTHING && fills_any(cell, w.pier) {
            fits &= spans.push(w.pier, PIER);
        }
    }
    (spans, fits)
}

/// What the ways write in block `z` at `cell` of a column whose spans are
/// `spans`: a deck's slab or its pier, the highest of those over it keeping
/// what two share, a slab before its pier; else the first air over it
/// (`AIR_OF` over its way's look); else `NOTHING`.
pub fn at(spans: &Spans, cell: Zoom, z: i32) -> u32 {
    let (mut best, mut look, mut air) = ([i32::MIN, i32::MIN], NOTHING, NOTHING);
    let mut k = 0;
    while k < spans.n as usize {
        let (run, v) = (spans.run[k], spans.value[k]);
        if fills(cell, run, z) {
            if v & AIR_OF != 0 {
                if air == NOTHING {
                    air = v;
                }
            } else if at_or_after(run, best) {
                (best, look) = (run, v);
            }
        }
        k += 1;
    }
    if look != NOTHING {
        look
    } else {
        air
    }
}

/// What the ways leave in a column's blocks, as a reader holds them: block
/// `z` of column `c` at the reader's zoom, what `at` answers there.
pub trait Airs {
    fn at(&self, c: [i32; 2], z: i32) -> u32;
}

/// The look of the way whose air `v` is, `NOTHING` where it is none.
fn air_way(v: u32) -> u32 {
    if v != NOTHING && v & AIR_OF != 0 {
        v & !AIR_OF
    } else {
        NOTHING
    }
}

/// What the ways write in block `b`, its own column read off its runs
/// (`Airs`): a deck's slab or its pier; else a bore's air (`AIR`); else, where
/// a cave carves ground the grid fills (`carved`), the way's block about the
/// air a way leaves beside it — the column's own under and over it, the four
/// across its faces, in the grid's order, the first that is a way's air — so
/// a cave never opens into a way; else `NOTHING`, the block as the ground and
/// its caves answer it. Ground no cave carves keeps its look: the ways write
/// only what they change. The seven read in one loop (`near`), as a GPU
/// inlines a read once a place it is called from.
pub fn block(airs: &impl Airs, b: [i32; 3], carved: bool) -> u32 {
    let mut k = 0;
    while k < if carved { 7 } else { 1 } {
        let d = near(k);
        let v = airs.at([b[0] + d[0], b[1] + d[1]], b[2] + d[2]);
        if k == 0 && v != NOTHING {
            return if v & AIR_OF != 0 { AIR } else { v };
        }
        if k > 0 && air_way(v) != NOTHING {
            return air_way(v);
        }
        k += 1;
    }
    NOTHING
}

/// The `k`-th of a block and the six about it, from its own: the row under
/// it, the cell before it, the block under it, the block over it, the cell
/// after it, the row over it — the order of the grid's cells.
fn near(k: u32) -> [i32; 3] {
    let mut d = [0; 3];
    if k > 0 {
        let j = k - 1;
        let (i, s) = if j < 3 { (j, -1) } else { (5 - j, 1) };
        d[if i == 2 { 2 } else { 1 - i as usize }] = s;
    }
    d
}

/// A run along z as a square holds it, a word: its first block, an `i16`,
/// and its value (`at`'s, its air `AIR_OF` over its way's look), `NOTHING`
/// as `0xffff`. It holds to the next run's first block.
pub fn run_word(lo: i32, value: u32) -> u32 {
    (lo as u32 & 0xffff) | (value & 0xffff) << 16
}

/// A run's first block and its value; a value of no way is `NOTHING`.
pub fn run_of(w: u32) -> (i32, u32) {
    let value = w >> 16;
    ((w << 16) as i32 >> 16, if value == 0xffff { NOTHING } else { value })
}

/// The first block past `z` where one of a column's spans' blocks
/// (`filled`) begins or ends, `i32::MAX` where none does.
fn edge_after(spans: &Spans, cell: Zoom, z: i32) -> i32 {
    let mut edge = i32::MAX;
    let mut k = 0;
    while k < spans.n as usize {
        let [first, last] = filled(cell, spans.run[k]);
        if first > z {
            edge = edge.min(first);
        }
        if last >= z {
            edge = edge.min(last + 1);
        }
        k += 1;
    }
    edge
}

/// A column's runs at `cell` as a square holds them (pxl-portable window.rs
/// `RUNS_AT`): each block from which what the ways write (`at`) changes, as
/// a run (`run_word`), lowest first, the last `NOTHING` on; the `k`-th to
/// `put`, and how many. Between two edges of its spans' blocks (`filled`)
/// the same spans fill every block, so `at` is read once a piece, never a
/// block at a time: of `n` spans, at most `2n` runs. The walk steps from
/// edge to edge in one place, as a GPU inlines a call once a site.
pub fn runs(spans: &Spans, cell: Zoom, mut put: impl FnMut(u32, u32)) -> u32 {
    let (mut z, mut held, mut n) = (i32::MIN, NOTHING, 0);
    loop {
        z = edge_after(spans, cell, z);
        if z == i32::MAX {
            return n;
        }
        let v = at(spans, cell, z);
        if v != held {
            put(n, run_word(z, v));
            (held, n) = (v, n + 1);
        }
    }
}
