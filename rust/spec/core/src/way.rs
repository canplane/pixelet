// pxl-spec-core/way.rs

//! gpu-portable: the ways off the ground, as a cell asks them — a bridge's
//! deck and its piers, a tunnel's bore and the way's ground about it, an
//! approach's embankment or cutting — one rule for a square's fill, which
//! keeps each column's spans once and builds its ways' tree off them, which
//! the descent on both sides reads (pxl-spec bake/separation.rs keeps the
//! lines, solved offline). A cell is asked of the lines over its column,
//! each visit of a line its own crossing: the run of its segments, one
//! after another, that are over the column. A column's visits make its
//! spans once (`column`), what a cell of it holds is read off them (`at`),
//! and ground beside the air they leave takes that way's look, read off the
//! spans of the columns beside it as its fill builds its tree (`lined`), so
//! a reader reads a cell's own column alone. What is under half a cell in any dimension at a
//! zoom is absent there, as the fold drops a line under half a cell wide: a
//! slab or a bore fills a cell where it holds half its height or more, and
//! a pier, a leaf cell square, stands only in columns no more than two leaf
//! cells wide. Index loops, no `Option` (a crossing none is is `Crossing::NONE`), words
//! for tags, nothing a GPU lacks (`hypot`).

pub mod tree;

use crate::cube::{sentinel, Ground};
use crate::param::tile::{LEAF_ZOOM, TOP_ZOOM};
use crate::param::way::BEAM_PILLAR_INTERVAL_M;
use crate::param::way::DECK_THICKNESS_M;
use crate::param::way::LINE_PAST;
use crate::param::way::PIER_MIN_RISE_M;
use crate::param::way::STACKED_DECK_HEADROOM_M;
use crate::param::way::WIDE_BEAM_PIER_INTERVAL_M;
use crate::{cells_of_m, Zoom, LEAF_CELL};

/// Points are eighths of the tile's cell.
pub const POINT_SCALE: f32 = 8.0;

// From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
// the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
// on a deck's line rather than place Minecraft blocks.

/// Arnis's `LOWER_DECK_GAP`, a cell short of the stacked headroom: a deck at
/// least this far under a pier's blocks it.
const LOWER_DECK_GAP_M: f32 = STACKED_DECK_HEADROOM_M - 1.0;

/// What a pier is drawn as.
pub const PIER: u32 = Ground::building as u32;

/// A line's form, as its byte on the wire.
pub const BRIDGE: u32 = 0;
pub const TUNNEL: u32 = 1;
pub const APPROACH: u32 = 2;

/// What `lined` answers where the ways write nothing there.
pub const NOTHING: u32 = u32::MAX;

/// What a bore's air is where the ways write it (`lined`): the ground's void.
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
    while z > TOP_ZOOM && weight >= (1u32 << (2 * (LEAF_ZOOM - z + 1))) as f32 {
        z -= 1;
    }
    z
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
    pub fn words(
        [a, b]: [[i16; 2]; 2],
        tops: [i16; 2],
        [width, kind, flags, clear]: [u8; 4],
    ) -> [u32; SEGMENT_WORDS] {
        [
            pair_word(a[0], a[1]),
            pair_word(b[0], b[1]),
            pair_word(tops[0], tops[1]),
            u32::from(width)
                | u32::from(kind) << 8
                | u32::from(flags) << 16
                | u32::from(clear) << 24,
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

/// What a square's lines open with (pxl-spec-core window.rs `LINES_*`): its
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
        phase: [
            [f32::from_bits(w[3]), f32::from_bits(w[4])],
            [f32::from_bits(w[5]), f32::from_bits(w[6])],
        ],
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
pub fn pier_covers(
    st: &Stations,
    width: f32,
    a: [f32; 2],
    d: [f32; 2],
    near: [f32; 2],
    at: [f32; 2],
    side: f32,
) -> bool {
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
    let (off, m) = if leaves >= 2.0 {
        (leaf / 2.0, 4)
    } else {
        (0.0, 1)
    };
    let (sides, n) = bent(half);
    let mut covered = false;
    let mut j = 0;
    while j < m {
        let p = [
            at[0] + if j & 1 == 0 { -off } else { off },
            at[1] + if j & 2 == 0 { -off } else { off },
        ];
        let rel = [p[0] - foot[0], p[1] - foot[1]];
        let along = (rel[0] * dir[0] + rel[1] * dir[1]).abs();
        let across = rel[0] * dir[1] - rel[1] * dir[0];
        // an edge column stands a leaf cell in from the edge
        let mut i = 0;
        while i < n as usize {
            covered |= along <= leaf / 2.0
                && (across - (sides[i] * (half - 1).max(0)) as f32 * leaf).abs() <= leaf / 2.0;
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
    let t = if len2 > 0.0 {
        ((at[0] - a[0]) * d[0] + (at[1] - a[1]) * d[1]) / len2
    } else {
        0.0
    };
    let cut = (ends[0] && t < 0.0) || (ends[1] && t > 1.0);
    let t = t.clamp(0.0, 1.0);
    let near = [a[0] + d[0] * t, a[1] + d[1] * t];
    (
        t,
        !cut && covers(width, length([at[0] - near[0], at[1] - near[1]])),
    )
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
    let reach = if len2 > 0.0 {
        side / 2.0 * (d[0].abs() + d[1].abs()) / len2
    } else {
        0.0
    };
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
            let pier = if c.pier == 1 && lo > ground {
                [ground, lo]
            } else {
                none
            };
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
    /// The cells' zoom, and leaf cells a ground metre over the ground.
    fn cell(&self) -> Zoom;
    fn per_m(&self) -> f32;
    /// How many segments may be over column `c`, and the `k`-th of them.
    fn count(&self, c: [i32; 2]) -> u32;
    fn segment(&self, c: [i32; 2], k: u32) -> Segment;
    /// The column's center in its tile's cells, a cell a side.
    fn center(&self, c: [i32; 2]) -> [f32; 2];
    fn stations(&self) -> Stations;
    fn look(&self, c: [i32; 2]) -> u32;
    /// Whether the area's edge walls the column: ground beside cells past
    /// the area (pxl-spec-core `area::laid`), where no way writes, as none
    /// does past it.
    fn walled(&self, c: [i32; 2]) -> bool;
    fn face(&self, c: [i32; 2]) -> i32;
    /// Whether the `k`-th segment over column `c` is the next of its line
    /// after the one before it there, so the two are one visit.
    fn joins(&self, c: [i32; 2], k: u32) -> bool;
}

/// The `k`-th segment's crossing of column `c`.
fn crossing_at<C: Columns>(cols: &C, c: [i32; 2], k: u32) -> Crossing {
    crossing(&cols.segment(c, k), cols.center(c), 1.0, &cols.stations())
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

/// Whether the leaf cells `[lo, hi)` fill cell `z` of the grid's zoom: more
/// than half its height, as its share of the cell rounds (half to even), so
/// a run of half a cell or thinner fills none.
pub fn fills(cell: Zoom, [lo, hi]: [i32; 2], z: i32) -> bool {
    let d = cell as i32 - LEAF_CELL as i32;
    if d >= 0 {
        let leaf = z >> d;
        return lo <= leaf && leaf < hi;
    }
    let s = 1 << -d;
    2 * (hi.min((z + 1) * s) - lo.max(z * s)) > s
}

/// The cells of the grid's zoom the leaf cells `[lo, hi)` fill (`fills`),
/// `[first, last]`, first past last where none: every cell between the
/// first and the last they reach is filled whole, so only those two can
/// fail, and a cell finer than a leaf fills as its leaf does.
pub fn filled(cell: Zoom, [lo, hi]: [i32; 2]) -> [i32; 2] {
    let d = LEAF_CELL as i32 - cell as i32;
    if lo >= hi {
        return [1, 0];
    }
    if d <= 0 {
        return [lo << -d, (hi << -d) - 1];
    }
    let (first, last) = (lo >> d, (hi - 1) >> d);
    [
        if fills(cell, [lo, hi], first) {
            first
        } else {
            first + 1
        },
        if fills(cell, [lo, hi], last) {
            last
        } else {
            last - 1
        },
    ]
}

/// Whether the leaf cells `[lo, hi)` fill any cell of the grid's zoom.
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
fn pier_stands<C: Columns>(
    cols: &C,
    c: [i32; 2],
    pier: [i32; 2],
    ground: i32,
    per_m: f32,
) -> [i32; 2] {
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

/// What a span's value marks as a bore's air, over the look of its way.
pub(crate) const AIR_OF: u32 = 0x80;

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
/// occupies over the column's face `face`, `per_m` leaf cells a metre, its
/// pier where one stands.
fn writes<C: Columns>(cols: &C, c: [i32; 2], k: u32, (face, per_m): (i32, f32)) -> (Writes, u32) {
    let (x, next) = visit(cols, c, k);
    if !x.is() || !Ground::grounds(x.kind) {
        return (Writes::NONE, next);
    }
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
            pier: if empty(o.pier) {
                o.pier
            } else {
                pier_stands(cols, c, o.pier, face, per_m)
            },
        },
        // An approach's look, laid into the cells by the bake (pxl-spec bake/way.rs `lay`).
        Occupy::Grade => Writes::NONE,
    };
    (w, next)
}

/// The most spans one column's visits make (`column`): the most of the
/// format-12 set (2026-10-17), measured with every tile at every zoom (z14's
/// 10 at 01033210230311; z15's most 9, none past z11); a tile past it fails
/// its bake (pxl-spec bake/tile/terrain.rs `fill_fit`).
pub const SPANS: usize = 10;

/// A column's spans, in leaf cells, in the order its visits come: each a
/// deck's slab or an embankment (its way's look), a pier (`PIER`), or a
/// bore's or a cutting's air (`AIR_OF` over its way's look); `n` of them.
/// And its face cell (`ground::face_cell`), under which a bore's air
/// beside it lines it (`lined`), `i32::MIN` where the area's edge walls it
/// or it is past the area, where nothing does. What its cells are read
/// off, its visits found once: a square's fill keeps them (pxl-spec-core
/// fill.rs `spans`, read through `Kept`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spans {
    pub run: [[i32; 2]; SPANS],
    pub value: [u32; SPANS],
    pub n: u32,
    pub face: i32,
}

impl Spans {
    pub const NONE: Spans = Spans {
        run: [[0, 0]; SPANS],
        value: [0; SPANS],
        n: 0,
        face: i32::MIN,
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
/// cell of the grid's zoom (`fills_any`), as `at` reads no other, and its
/// face cell — none, nor a face, in a column of the bedrock, past the area
/// or walled by its edge (`Columns::walled`), as bedrock wins over a way as
/// it does over the ground's face — and whether they all fit `SPANS`.
pub fn column<C: Columns>(cols: &C, c: [i32; 2]) -> (Spans, bool) {
    let (mut spans, mut fits) = (Spans::NONE, true);
    if cols.look(c) == crate::ground::UNKNOWN || cols.walled(c) {
        return (spans, fits);
    }
    // The column's face and the tile's scale, read once for all its visits.
    let cell = cols.cell();
    let ground = (cols.face(c), cols.per_m());
    spans.face = crate::ground::face_cell(cell, ground.0);
    let mut k = 0;
    while k < cols.count(c) {
        let (w, next) = writes(cols, c, k, ground);
        k = next;
        if w.value != NOTHING && fills_any(cell, w.run) {
            fits &= spans.push(w.run, w.value);
        }
        if w.value != NOTHING && fills_any(cell, w.pier) {
            fits &= spans.push(w.pier, PIER);
        }
    }
    (spans, fits)
}

/// A square's columns' spans and the ring's about it as its fill keeps
/// them (pxl-spec-core fill.rs `spans`), read in place: column `c`'s face
/// cell and how many spans it has (`Spans`), and its `j`-th span and its
/// value.
pub trait Kept {
    fn head(&self, c: [i32; 2]) -> (i32, u32);
    fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32);
}

/// What the ways write in cell `z` at zoom `cell` of column `c`, its `n` spans
/// kept in `k`: a deck's slab or its pier, the highest of those over it
/// keeping what two share, a slab before its pier; else the first air over
/// it (`AIR_OF` over its way's look); else `NOTHING`.
fn at(k: &impl Kept, c: [i32; 2], n: u32, cell: Zoom, z: i32) -> u32 {
    let (mut best, mut look, mut air) = ([i32::MIN, i32::MIN], NOTHING, NOTHING);
    let mut j = 0;
    while j < n {
        let (run, v) = k.span(c, j);
        if fills(cell, run, z) {
            if v & AIR_OF != 0 {
                if air == NOTHING {
                    air = v;
                }
            } else if at_or_after(run, best) {
                (best, look) = (run, v);
            }
        }
        j += 1;
    }
    if look != NOTHING {
        look
    } else {
        air
    }
}

/// The `k`-th of a cell and the six about it, from its own: the row under
/// it, the column before it, the cell under it, the cell over it, the column
/// after it, the row over it — the order of the grid's cells. No index into
/// an array: a bounds check's panic is a return no barrier after it may
/// follow (pxl-shader `way_tree_cs`, whose leaves read it).
fn near(k: u32) -> [i32; 3] {
    // Its axis, y, x then z, and its side.
    let (i, s) = if k < 4 {
        (k.wrapping_sub(1), -1)
    } else {
        (6 - k, 1)
    };
    let on = |axis: u32| if k != 0 && i == axis { s } else { 0 };
    [on(1), on(0), on(2)]
}

/// Where the ways may fill a node along z, at the zooms from `coarsest` to
/// `near` a reader asks within it, where what they fill under it lies in
/// the leaf cells `[lo, hi)` (`ground::Bounds`, a bore's air none): from
/// its lowest to past its highest, in leaf cells. A cell fills where more
/// than half of it is (`fills`): at a zoom whose cell is `s` leaf cells,
/// from the first cell more than half over `lo` to past the last more than
/// half under `hi`; at the leaves or finer, `[lo, hi)` itself, a cell
/// filling as its leaf does. Nothing (`ground::EMPTY`) where they fill
/// nothing.
///
/// The node's bottom and top are whole cells of every zoom finer than it,
/// so none of its cells fills exactly where it does not meet the finest
/// zoom's slab (`ground::meets`), and a coarser zoom's cell, more than
/// twice its, fills less still within it.
pub fn slab([lo, hi]: [i32; 2], [coarsest, near]: [Zoom; 2]) -> [i32; 2] {
    if lo >= hi {
        return crate::ground::EMPTY;
    }
    let [mut under, mut over] = crate::ground::EMPTY;
    let mut z = coarsest;
    while z <= near {
        let (a, b) = if z < LEAF_CELL {
            let k = LEAF_CELL - z;
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

/// The column the `m`-th of `near` reads beside column `c`, and the cell
/// it reads it at from `z`: its own for the cells under and over.
fn beside(c: [i32; 2], m: u32, z: i32) -> ([i32; 2], i32) {
    let d = near(m);
    ([c[0] + d[0], c[1] + d[1]], z + d[2])
}

/// What the ways write in cell `z` of column `c` at zoom `cell`, its spans and
/// those beside it kept in `k` (`at` of its own): a deck's slab or its
/// pier, else a bore's air (`AIR`); else, under its face cell, where a
/// cave may take it (`ground::face_cell`), the way's look where a bore's
/// air is beside it — under or over it, or across one of its four faces,
/// the first in `near`'s order — so a cave never opens into a way, whatever
/// carves the ground
/// (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md,
/// "Separation"); else `NOTHING`. Read off the spans in place, the
/// column's own first: a cell it writes reads none beside it.
pub fn lined(k: &impl Kept, c: [i32; 2], cell: Zoom, z: i32) -> u32 {
    let (face, n) = k.head(c);
    let v = at(k, c, n, cell, z);
    if v != NOTHING {
        return if v & AIR_OF != 0 { AIR } else { v };
    }
    let mut m = 1;
    while z < face && m < 7 {
        let (b, zb) = beside(c, m, z);
        let v = at(k, b, k.head(b).1, cell, zb);
        if v != NOTHING && v & AIR_OF != 0 {
            return v & !AIR_OF;
        }
        m += 1;
    }
    NOTHING
}

/// The leaf cells between which column `c`'s cells at zoom `cell` hold what
/// the ways write there (`lined`), its ground's top `top` leaf cells: from
/// the lowest to past the highest, `ground::EMPTY` where they write none.
/// Its own spans' cells (`filled`), a bore's air only as far as the top,
/// over which it is air whatever writes it; and under its face cell, the
/// cells a bore's air beside it lines, where no slab or pier of that
/// column fills the cell read (`bare`). A lined cell is under the face's
/// cell, so under the top: the hull of the cells `lined` reads something
/// in, exactly, off the spans, never a cell at a time.
pub fn bound(k: &impl Kept, c: [i32; 2], cell: Zoom, top: i32) -> [i32; 2] {
    let s = 1i32 << (LEAF_CELL - cell);
    let (face, n) = k.head(c);
    let [mut lo, mut hi] = crate::ground::EMPTY;
    let mut j = 0;
    while j < n {
        let (run, v) = k.span(c, j);
        let [first, last] = filled(cell, run);
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
    // A face of none lines nothing, and has no cell under it to name.
    let mut m = if face == i32::MIN { 7 } else { 1 };
    while m < 7 {
        let (b, dz) = beside(c, m, 0);
        let nb = k.head(b).1;
        let mut j = 0;
        while j < nb {
            let (run, v) = k.span(b, j);
            // Air read at `z + dz` lines `z`, under the face cell, where no
            // slab or pier of its column fills it.
            let [first, last] = filled(cell, run);
            let last = last.min(face - 1 + dz);
            let (from, to) = (
                bare(k, b, nb, cell, [first, last], 1),
                bare(k, b, nb, cell, [last, first], -1),
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

/// The first cell from `from` toward `to`, a `step` at a time, that no
/// slab or pier of column `b`'s `n` spans fills, where its air is read as
/// air (`at`); past `to` where they fill every one. Each step past one
/// of them, so no more than one a span.
fn bare(k: &impl Kept, b: [i32; 2], n: u32, cell: Zoom, [from, to]: [i32; 2], step: i32) -> i32 {
    let (mut z, mut moved) = (from, true);
    while moved && (z - to) * step <= 0 {
        moved = false;
        let mut j = 0;
        while j < n {
            let (run, v) = k.span(b, j);
            let [first, last] = filled(cell, run);
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

    /// A column and the four beside it, each a face cell and its spans.
    struct Five([(i32, [([i32; 2], u32); 4], u32); 5]);

    impl Five {
        fn of(&self, c: [i32; 2]) -> &(i32, [([i32; 2], u32); 4], u32) {
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

    impl Kept for Five {
        fn head(&self, c: [i32; 2]) -> (i32, u32) {
            let (face, _, n) = self.of(c);
            (*face, *n)
        }
        fn span(&self, c: [i32; 2], j: u32) -> ([i32; 2], u32) {
            self.of(c).1[j as usize]
        }
    }

    /// `bound` is the hull of the cells `lined` writes, a cell at a
    /// time, a bore's air held to the top: every way a slab, a pier or air
    /// over another, beside it or over and under it can lie, at zooms from
    /// the leaves to three coarser.
    #[test]
    fn the_bound_is_the_hull_of_what_lined_writes() {
        let mut seed = 0x2545_f491_u32;
        let mut next = |m: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % m
        };
        for _ in 0..20_000 {
            let cell = LEAF_CELL - next(4);
            let s = 1i32 << (LEAF_CELL - cell);
            let top = next(120) as i32 - 60;
            let face = crate::ground::face_cell(
                cell,
                crate::ground::face_height(cell, crate::ground::cells(cell, top as f32)),
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
            let mut hull = crate::ground::EMPTY;
            for z in -200 / s..200 / s {
                let v = lined(&k, [0, 0], cell, z);
                if v == NOTHING {
                    continue;
                }
                let end = if v == AIR {
                    ((z + 1) * s).min(top)
                } else {
                    (z + 1) * s
                };
                if z * s < end {
                    hull = [hull[0].min(z * s), hull[1].max(end)];
                }
            }
            assert_eq!(
                bound(&k, [0, 0], cell, top),
                hull,
                "{:?} at {cell}, top {top}",
                k.0
            );
        }
    }
}
