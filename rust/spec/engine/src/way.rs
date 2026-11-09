// pxl-engine/way.rs

//! gpu-portable: the ways off the ground, as a cell asks them: a bridge's deck and its piers,
//! a tunnel's bore and the way's ground around it, and an approach's embankment or cutting.
//! It is one rule for a brick's fill, which keeps each column's spans once and builds the
//! brick's ways' tree from them; the descent, on the CPU and on the GPU, reads that tree. The
//! lines themselves are solved offline (pxl-spec bake/separation.rs).
//!
//! A cell is asked of the lines over its column, each visit of a line being its own crossing:
//! the run of the line's segments, one after another, that lie over the column. A column's
//! visits make its spans once (`column`), and what a cell of the column holds is read from
//! them (`at`). Ground beside the air the spans leave takes that way's palette index, read
//! from the spans of the columns beside it as the fill builds the tree (`way_cell`), so a
//! reader reads only a cell's own column. Whatever is under half a cell in any dimension at a
//! zoom is absent at that zoom, as the fold drops a line under half a cell wide: a slab or a
//! bore fills a cell where it holds half the cell's height or more, and a pier, one leaf cell
//! across, stands only in columns no more than two leaf cells wide.
//!
//! The code uses index loops and no `Option` (a missing crossing is `Crossing::NONE`), and
//! words for tags. Where a column is crossed, its road's tops and its pier (`on_way`,
//! `crossing`, `pier_covers`) are exact, in integers over the wire's own (points in eighths of
//! a cell, tops and stations in leaf cells), so the CPU and a GPU decide them alike whatever
//! they fuse or reorder. There is no division but a rational's
//! floor, and the one root, a segment's direction, is rounded once by the CPU and handed to a
//! GPU in a word (`Segment::dir`).

use crate::ground;
use crate::param::way::BEAM_PILLAR_INTERVAL_M;
use crate::param::way::DECK_THICKNESS_M;
use crate::param::way::LINE_PAST;
use crate::param::way::PIER_MIN_RISE_M;
use crate::param::way::STACKED_DECK_HEADROOM_M;
use crate::param::way::WIDE_BEAM_PIER_INTERVAL_M;
use crate::value;
use crate::{cells_of_m, Zoom, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM};

/// Points are counted in eighths of the tile's cell.
pub const POINT_SCALE: f32 = 8.0;

// From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
// the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
// on a deck's line rather than place Minecraft blocks.

/// A deck at least this far under a pier blocks it: one cell short of the stacked headroom
/// (Arnis's `LOWER_DECK_GAP`).
pub const LOWER_DECK_GAP_M: f32 = STACKED_DECK_HEADROOM_M - 1.0;

/// What a pier is drawn as.
pub(crate) const PIER: u32 = value::PIER;

/// A line's form, as its byte on the wire.
pub const BRIDGE: u32 = 0;
pub const TUNNEL: u32 = 1;
pub const APPROACH: u32 = 2;

/// What `way_cell` answers where the ways write nothing there.
pub const NOTHING: u32 = u32::MAX;

/// What a bore's air is where the ways write it (`way_cell`): the ground's void.
pub const AIR: u32 = value::VOID;

/// A length in a tile's cells as eighths (`POINT_SCALE`), rounded half to even, for a point
/// or a width that the wire has not already put in eighths, as the rule takes them
/// (`on_way`).
pub fn eighths(v: f64) -> i32 {
    libm::rint(v * f64::from(POINT_SCALE)) as i32
}

/// Whether a way of `width` cells vanishes at those cells: its width rounds to no cell (half
/// to even, so half a cell is none), which is what a fold discards.
pub fn vanishes(width: f32) -> bool {
    libm::rintf(width) == 0.0
}

/// The coarsest zoom at which a line's point is held, from its weight: its effective area on
/// its whole line, in leaf cells² (Visvalingam–Whyatt, as TopoJSON's presimplify and
/// mapshaper weigh a point). A zoom holds the points that weigh one of its own cells or more,
/// so what a zoom keeps depends on each point alone and is the same in every tile holding it.
/// A line's ends weigh without bound and are held at every zoom.
pub fn coarsest(weight: f32) -> Zoom {
    let mut z = LEAF_ZOOM;
    while z > MIN_ZOOM && weight >= (1u32 << (2 * (LEAF_ZOOM - z + 1))) as f32 {
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
pub fn half_width(width: f32) -> f32 {
    (width * 0.5).max(core::f32::consts::FRAC_1_SQRT_2)
}

/// How far from its line a way `width` eighths of a cell wide covers a cell's
/// center, squared, in sixteenths of a cell: half its width, and at least
/// half a cell's diagonal, `½ · 16²`, so a diagonal way stays
/// side-connected (`half_width`).
pub fn reach2(width: u32) -> i64 {
    let w = i64::from(width);
    (w * w).max(128)
}

/// `base + num / den` rounded half to even, `den > 0`: exact, in integers.
pub fn nearest(num: i64, den: i64, base: i32) -> i32 {
    let (q, r) = (num.div_euclid(den), num.rem_euclid(den));
    let z = i64::from(base) + q;
    let up = 2 * r > den || 2 * r == den && z & 1 == 1;
    (z + i64::from(up)) as i32
}

/// How far from its line a way reaches anything drawn, in its tile's cells: its half width
/// (`half_width`) and one cell more. So a tile's lines and a brick's hold every line whose air
/// or slab can stand in the ring of cells around them, a thin way's included.
pub fn footprint(width: f32) -> f32 {
    half_width(width) + LINE_PAST
}

/// A deck's slab in leaf cells, at least one, where `per_m` is the leaf cells a meter.
pub(crate) fn slab_depth(per_m: f32) -> i32 {
    cells_of_m(DECK_THICKNESS_M, per_m).max(1)
}

/// One segment of a line, in eighths of its tile's cell (`POINT_SCALE`):
/// from `a` to `b`, its road surface `tops` there in leaf cells, its width,
/// the palette index it shows, its form, its tunnel's clear height in leaf
/// cells, whether it is its line's first or last, where the line ends
/// square, and whether the one stored before it is its line's one before
/// it; and its direction (`dir`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: [i32; 2],
    pub b: [i32; 2],
    pub tops: [i32; 2],
    pub width: u32,
    pub kind: u32,
    pub form: u32,
    pub clear: u32,
    pub first: u32,
    pub last: u32,
    pub continues: u32,
    pub dir: [i32; 2],
}

/// A segment's words: `a` and `b` each two `i16` eighths of a cell, the tops
/// two `i16`, then width (eighths), kind, form with first, last and
/// continues over it, and clear, a byte each; then its direction, two `i16`,
/// at `SEGMENT_DIR`.
pub const SEGMENT_WORDS: usize = SEGMENT_DIR + 1;
pub const SEGMENT_DIR: usize = 4;

/// A direction's bits: a unit vector times `1 << DIR_BITS`, which an `i16`
/// holds.
pub const DIR_BITS: u32 = 14;

/// The axis a run `d` is longest along, x where the two are equal.
fn major(d: [i32; 2]) -> usize {
    if d[0].abs() >= d[1].abs() {
        0
    } else {
        1
    }
}

/// Run `d`'s direction: `d` divided by its length, times `1 << DIR_BITS`, each taken in
/// doubles and rounded half to even, and none for a run of no length. Only the CPU computes
/// it, since it takes a root no integer holds, and hands it to a GPU in a segment's words
/// (`Segment::words`). A pier's section is measured along it (`pier_covers`).
pub fn dir(d: [i32; 2]) -> [i32; 2] {
    let [x, y] = d.map(f64::from);
    let length = libm::sqrt(x * x + y * y);
    if length == 0.0 {
        return [0, 0];
    }
    let one = f64::from(1u32 << DIR_BITS);
    [
        libm::rint(x * one / length) as i32,
        libm::rint(y * one / length) as i32,
    ]
}

/// A segment's form with whether it is its line's first and last and
/// continues the one stored before it, a byte.
pub fn flags(form: u32, first: bool, last: bool, continues: bool) -> u8 {
    ((form & 3) | u32::from(first) << 2 | u32::from(last) << 3 | u32::from(continues) << 4) as u8
}

/// A word's two `i16`, sign-extended, as a GPU has no `i16`.
fn i16_pair(w: u32) -> [i32; 2] {
    [(w << 16) as i32 >> 16, w as i32 >> 16]
}

fn pair_word(lo: i16, hi: i16) -> u32 {
    u32::from(lo as u16) | u32::from(hi as u16) << 16
}

impl Segment {
    pub fn of(w: [u32; SEGMENT_WORDS]) -> Segment {
        let byte = |k: u32| (w[3] >> (8 * k)) & 0xff;
        Segment {
            a: i16_pair(w[0]),
            b: i16_pair(w[1]),
            tops: i16_pair(w[2]),
            width: byte(0),
            kind: byte(1),
            form: byte(2) & 3,
            first: (byte(2) >> 2) & 1,
            last: (byte(2) >> 3) & 1,
            continues: (byte(2) >> 4) & 1,
            clear: byte(3),
            dir: i16_pair(w[SEGMENT_DIR]),
        }
    }

    /// Its words, from the wire's integers: its two points in eighths, its tops in leaf cells,
    /// and its bytes (width in eighths, kind, `flags`, clear); then its run's direction (`dir`).
    pub fn words(
        [a, b]: [[i16; 2]; 2],
        tops: [i16; 2],
        [width, kind, flags, clear]: [u8; 4],
    ) -> [u32; SEGMENT_WORDS] {
        let d = dir([0, 1].map(|k| i32::from(b[k]) - i32::from(a[k])));
        [
            pair_word(a[0], a[1]),
            pair_word(b[0], b[1]),
            pair_word(tops[0], tops[1]),
            u32::from(width)
                | u32::from(kind) << 8
                | u32::from(flags) << 16
                | u32::from(clear) << 24,
            pair_word(d[0] as i16, d[1] as i16),
        ]
    }
}

/// Where a tile's piers may stand, in leaf cells: the bents' lattice along each axis, with a
/// step of `step[w]` for the narrow beam (`w` 0) and the wide (1), and where the tile's
/// corner lies inside a step (`phase[w][axis]`), so a station is the world's and not the
/// tile's; and the leaf cells a tile's cell spans a side (`leaf`), in which a pier's section
/// is measured. All are whole leaf cells, as every length given in meters is (`cells_of_m`),
/// so a station is exact. It is made once a tile (pxl-spec procedural.rs `Frame::stations`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StationGrid {
    pub step: [i32; 2],
    pub phase: [[i32; 2]; 2],
    pub leaf: u32,
}

/// A bent's spacing for the narrow beam (`w` 0) and the wide (1), in meters.
pub const INTERVAL_M: [f32; 2] = [BEAM_PILLAR_INTERVAL_M, WIDE_BEAM_PIER_INTERVAL_M];

/// What a brick's lines open with (pxl-engine clipmap.rs `LINES_*`): its tile's leaf cells a
/// ground meter (pxl-spec `Frame::per_m`), as a float's bits, then its stations
/// (`StationGrid`), a word each: the narrow beam's step and then the wide's, the phase
/// `phase[w][axis]` at `HEADER_PHASE + 2w + axis`, and the leaf cells a cell.
pub const HEADER_PER_M: usize = 0;
pub const HEADER_STEP: usize = 1;
pub const HEADER_PHASE: usize = 3;
pub const HEADER_LEAF: usize = 7;
pub const HEADER_WORDS: u32 = 8;

pub fn header(per_m: f32, st: &StationGrid) -> [u32; HEADER_WORDS as usize] {
    let [[p0, p1], [p2, p3]] = st.phase;
    let mut w = [0; HEADER_WORDS as usize];
    w[HEADER_PER_M] = per_m.to_bits();
    w[HEADER_STEP] = st.step[0] as u32;
    w[HEADER_STEP + 1] = st.step[1] as u32;
    w[HEADER_PHASE] = p0 as u32;
    w[HEADER_PHASE + 1] = p1 as u32;
    w[HEADER_PHASE + 2] = p2 as u32;
    w[HEADER_PHASE + 3] = p3 as u32;
    w[HEADER_LEAF] = st.leaf;
    w
}

pub(crate) fn of_header(w: [u32; HEADER_WORDS as usize]) -> (f32, StationGrid) {
    let i = |k: usize| w[k] as i32;
    let st = StationGrid {
        step: [i(HEADER_STEP), i(HEADER_STEP + 1)],
        phase: [
            [i(HEADER_PHASE), i(HEADER_PHASE + 1)],
            [i(HEADER_PHASE + 2), i(HEADER_PHASE + 3)],
        ],
        leaf: w[HEADER_LEAF],
    };
    (f32::from_bits(w[HEADER_PER_M]), st)
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

/// Whether the column around `at`, a cell a side, holds the pier at the station nearest the
/// column's foot on segment `s`, at the share `n / len2` of it (`on_way`), counting from `a`
/// to before `b`; the interval is half-open, so a joint's station belongs to one segment.
/// Everything is in eighths of a cell. A pier is a brick one leaf cell across, its bent laid
/// out in leaf cells: a column more than two leaf cells wide holds none, since the pier is
/// under half of it, and a column of two holds it where one of its four leaf cells does.
///
/// It is exact, its bits counted for points within 2^16 eighths of each other and a step
/// under 2^12 eighths. The station nearest the foot is a rational's nearest integer
/// (`nearest`), its numerator under 2^51 and its quotient, the foot in steps, under 2^17 with
/// a step of an eighth or more; the station's share of the run, `u / d[k]`, is whole along
/// the major axis `k`. A leaf cell's center `p` lies `rel / d[k]` from the station's foot,
/// with `rel` whole: on the major axis `(p[k] − station) · d[k]`, and on the other
/// `(p[l] − a[l]) · d[k] − d[l] · u`, under 2^33. The bent is no more than 126 eighths from
/// the foot, so a center more than 128 from it on either axis holds none, and `rel` stays
/// under 2^23. Along and across the segment, `rel` against the direction (`dir`) stays under
/// 2^38, and each is compared with half a leaf cell times `d[k]`'s size times the direction's.
pub(crate) fn pier_covers(
    st: &StationGrid,
    s: &Segment,
    [n, len2]: [i64; 2],
    at: [i32; 2],
) -> bool {
    let d = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
    let (k, l) = (major(d), 1 - major(d));
    let leaf = st.leaf as i32;
    if d[k] == 0 || leaf > 2 {
        return false;
    }
    // A leaf cell, in eighths.
    let cell = POINT_SCALE as i32 / leaf;
    let half = half_cells((s.width * st.leaf) as f32 / POINT_SCALE);
    let w = if half <= 2 { 0 } else { 1 };
    let (step, phase) = (st.step[w] * cell, st.phase[w][k] * cell);
    // The station nearest the foot, `a + d · n / len2`, along the major axis.
    let foot = i64::from(phase + s.a[k]) * len2 + i64::from(d[k]) * n;
    let station = nearest(foot, i64::from(step) * len2, 0) * step - phase;
    let u = station - s.a[k];
    let on = if d[k] > 0 {
        0 <= u && u < d[k]
    } else {
        d[k] < u && u <= 0
    };
    if !on {
        return false;
    }
    // The centers of the leaf cells the column holds: its own where it is
    // one, else its four.
    let (off, m) = if leaf == 2 { (cell / 2, 4) } else { (0, 1) };
    let (sides, cnt) = bent(half);
    let (run, one) = (i64::from(d[k]), i64::from(d[k].abs()) << DIR_BITS);
    let reach = i64::from(cell / 2) * one;
    let mut covered = false;
    let mut j = 0;
    while j < m {
        let p = [
            at[0] + if j & 1 == 0 { -off } else { off },
            at[1] + if j & 2 == 0 { -off } else { off },
        ];
        let x = p[k] - station;
        let y = i64::from(p[l] - s.a[l]) * run - i64::from(d[l]) * i64::from(u);
        if x.abs() <= 128 && y.abs() <= 128 * run.abs() {
            let mut rel = [0; 2];
            (rel[k], rel[l]) = (i64::from(x) * run, y);
            let dir = s.dir.map(i64::from);
            let along = rel[0] * dir[0] + rel[1] * dir[1];
            let across = rel[0] * dir[1] - rel[1] * dir[0];
            // An edge column stands one leaf cell in from the edge.
            let mut i = 0;
            while i < cnt as usize {
                let side = (i64::from(sides[i] * (half - 1).max(0) * cell) * run) << DIR_BITS;
                covered |= along.abs() <= reach && (across - side).abs() <= reach;
                i += 1;
            }
        }
        j += 1;
    }
    covered
}

/// A segment over one column: the lowest and highest its road surface runs
/// across the column, in leaf cells, each rounded half to even, and whether
/// its pier's section covers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crossing {
    pub low: i32,
    pub top: i32,
    pub kind: u32,
    pub form: u32,
    pub pier: u32,
    pub clear: u32,
}

impl Crossing {
    /// No crossing: one of kind `NOTHING`.
    pub const NONE: Crossing = Crossing {
        low: 0,
        top: 0,
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

/// The road surface of `s` at the share `n / len2` of it, clamped to its ends, in leaf cells
/// rounded half to even: `tops[0] + Δ · n / len2`, whose numerator is under 2^51 and whose
/// quotient lies within the rise `Δ`, under 2^17, the tops each an `i16` (`nearest`).
fn surface(s: &Segment, n: i64, len2: i64) -> i32 {
    let rise = i64::from(s.tops[1] - s.tops[0]);
    nearest(rise * n.clamp(0, len2), len2, s.tops[0])
}

/// Where `at` lies along the segment from `a` to `b` of a way `width` eighths wide, all in
/// eighths of a cell, and whether the way covers `at` there (`reach2`). A line ends square
/// where it begins (`ends[0]`) and where it ends (`ends[1]`), as the ground cuts a way, and
/// its joints stay round. Where `at` lies is the share `n / len2` of the segment, clamped to
/// it, with `len2` the squared length; a segment of no length puts `at` at its start,
/// `[0, 1]`. The ground's cells and the ways' columns are covered by this alike.
///
/// It is exact, its bits counted for points within 2^16 eighths of each other (an `i16`'s
/// range) and a width under 2^8: `len2` and `n` stay under 2^34. Off an end, the end is
/// nearest, and the way covers `at` only within `max(width, 12)` sixteenths of it on each
/// axis, so its squared distance stays under 2^17. Between the ends, its squared distance is
/// `cross² / len2`, where `cross`, the run's cross product with `at`'s, is under 2^34; the
/// way covers it only where `2 · |cross| ≤ max(width, 12) · (|dx| + |dy|)`, which holds
/// `cross` under 2^24, so `4 · cross²` and `reach2 · len2` stay under 2^50.
pub fn on_way(
    [a, b]: [[i32; 2]; 2],
    width: u32,
    ends: [bool; 2],
    at: [i32; 2],
) -> ([i64; 2], bool) {
    let d = [i64::from(b[0] - a[0]), i64::from(b[1] - a[1])];
    let r = [i64::from(at[0] - a[0]), i64::from(at[1] - a[1])];
    let (len2, dot) = (d[0] * d[0] + d[1] * d[1], r[0] * d[0] + r[1] * d[1]);
    let cut = (ends[0] && dot < 0) || (ends[1] && dot > len2);
    let (reach, span) = (reach2(width), i64::from(width.max(12)));
    let near = |v: [i64; 2]| {
        2 * v[0].abs().max(v[1].abs()) <= span && 4 * (v[0] * v[0] + v[1] * v[1]) <= reach
    };
    let (n, covered) = if dot <= 0 {
        (0, near(r))
    } else if dot >= len2 {
        (len2, near([r[0] - d[0], r[1] - d[1]]))
    } else {
        let cross = r[0] * d[1] - r[1] * d[0];
        let l1 = d[0].abs() + d[1].abs();
        (
            dot,
            2 * cross.abs() <= span * l1 && 4 * cross * cross <= reach * len2,
        )
    };
    ([n, len2.max(1)], !cut && covered)
}

/// What segment `s` is over the column around `at`, a cell a side, in eighths of a cell, or
/// `NONE` unless its way covers `at` (`on_way`): the surface it runs through across the
/// column, and whether the column holds the section of a pier at one of its stations. Filling
/// a brick costs this once for each column and segment: some ten 64-bit products, and a
/// quotient for each of the two surfaces and for the station (`nearest`).
pub fn crossing(s: &Segment, at: [i32; 2], st: &StationGrid) -> Crossing {
    let ends = [s.first == 1, s.last == 1];
    let ([n, len2], on) = on_way([s.a, s.b], s.width, ends, at);
    if !on {
        return Crossing::NONE;
    }
    // The column spans half a cell, 4 eighths, either side of `at` along each
    // axis: `4 · (|dx| + |dy|) / len2` of the segment.
    let d = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
    let half = 4 * (i64::from(d[0].abs()) + i64::from(d[1].abs()));
    let (z0, z1) = (surface(s, n - half, len2), surface(s, n + half, len2));
    Crossing {
        low: z0.min(z1),
        top: z0.max(z1),
        kind: s.kind,
        form: s.form,
        pier: u32::from(s.form != TUNNEL && pier_covers(st, s, [n, len2], at)),
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
pub struct Occupancy {
    pub what: Occupy,
    pub span: [i32; 2],
    pub pier: [i32; 2],
}

/// What `c` occupies over ground `ground` leaf cells high, where `per_m` is the leaf cells a
/// ground meter: all that the surface sweeps across the column, so a slab or a bore steps
/// down to the next column's without a gap however steep the way.
pub fn occupies(c: &Crossing, ground: i32, per_m: f32) -> Occupancy {
    let (low, top) = (c.low, c.top);
    let none = [0, 0];
    match c.form {
        BRIDGE => {
            let lo = low - slab_depth(per_m);
            let pier = if c.pier == 1 && lo > ground {
                [ground, lo]
            } else {
                none
            };
            Occupancy {
                what: Occupy::Over,
                span: [lo, top],
                pier,
            }
        }
        TUNNEL => Occupancy {
            what: Occupy::Under,
            span: [low, top + c.clear as i32],
            pier: none,
        },
        // A way on the ground moves the ground with it: the ground is filled up to it or cut down to
        // it.
        _ if top > ground => Occupancy {
            what: Occupy::Over,
            span: [ground, top],
            pier: none,
        },
        _ if top < ground => Occupancy {
            what: Occupy::Under,
            span: [low, ground],
            pier: none,
        },
        _ => Occupancy {
            what: Occupy::Grade,
            span: none,
            pier: none,
        },
    }
}

/// The columns a brick's fill reads its ways from (`clipmap::Brick`): each column's segments,
/// in its own tile's cells, with its center there and the tile's stations, and its ground,
/// which is its palette index as read and its face's height in leaf cells. A column the brick
/// does not hold has no segment.
pub trait ColumnGrid {
    /// The cells' zoom, and leaf cells a ground meter over the ground.
    fn cell_zoom(&self) -> Zoom;
    fn per_m(&self) -> f32;
    /// How many segments may be over column `c`, and the `k`-th of them.
    fn cnt(&self, c: [i32; 2]) -> u32;
    fn segment(&self, c: [i32; 2], k: u32) -> Segment;
    /// The column's center in eighths of its tile's cells, a cell a side.
    fn center(&self, c: [i32; 2]) -> [i32; 2];
    fn stations(&self) -> StationGrid;
    fn palette_idx(&self, c: [i32; 2]) -> u32;
    /// Whether the column is the service area's wall (pxl-engine
    /// `area::is_wall`), where no way writes, as none does past it.
    fn is_walled(&self, c: [i32; 2]) -> bool;
    fn face(&self, c: [i32; 2]) -> i32;
    /// Whether the `k`-th segment over column `c` is the next of its line
    /// after the one before it there, so the two are one visit.
    fn joins(&self, c: [i32; 2], k: u32) -> bool;
}

/// The `k`-th segment's crossing of column `c`.
fn crossing_at<C: ColumnGrid>(cols: &C, c: [i32; 2], k: u32) -> Crossing {
    crossing(&cols.segment(c, k), cols.center(c), &cols.stations())
}

/// The visit of a line over column `c` starting at its `k`-th segment there: the run of
/// segments from it, each the next of its line and each over the column, taken as one
/// crossing (all the surface they sweep across the column, and a pier wherever any one's
/// station is), and where the next visit starts. It is `NONE` where the `k`-th segment is not
/// over the column.
fn visit<C: ColumnGrid>(cols: &C, c: [i32; 2], k: u32) -> (Crossing, u32) {
    let mut x = crossing_at(cols, c, k);
    if !x.is() {
        return (x, k + 1);
    }
    let mut j = k + 1;
    while j < cols.cnt(c) && cols.joins(c, j) {
        let y = crossing_at(cols, c, j);
        if !y.is() {
            break;
        }
        (x.low, x.top, x.pier) = (x.low.min(y.low), x.top.max(y.top), x.pier | y.pier);
        j += 1;
    }
    (x, j)
}

/// Whether the leaf cells `[lo, hi)` fill cell `z` at the grid's zoom: they fill more than
/// half its height, as their share of the cell rounds (half to even), so a run of half a cell
/// or thinner fills none.
pub fn fills(cell_zoom: Zoom, [lo, hi]: [i32; 2], z: i32) -> bool {
    let d = cell_zoom as i32 - LEAF_CELL_ZOOM as i32;
    if d >= 0 {
        let leaf = z >> d;
        return lo <= leaf && leaf < hi;
    }
    let s = 1 << -d;
    2 * (hi.min((z + 1) * s) - lo.max(z * s)) > s
}

/// The cells at the grid's zoom that the leaf cells `[lo, hi)` fill (`fills`), as
/// `[first, last]`, with first past last where none. Every cell between the first and the last
/// they reach is filled whole, so only those two can fail, and a cell finer than a leaf fills
/// as its leaf does.
pub fn fill_range(cell_zoom: Zoom, [lo, hi]: [i32; 2]) -> [i32; 2] {
    let d = LEAF_CELL_ZOOM as i32 - cell_zoom as i32;
    if lo >= hi {
        return [1, 0];
    }
    if d <= 0 {
        return [lo << -d, (hi << -d) - 1];
    }
    let (first, last) = (lo >> d, (hi - 1) >> d);
    [
        if fills(cell_zoom, [lo, hi], first) {
            first
        } else {
            first + 1
        },
        if fills(cell_zoom, [lo, hi], last) {
            last
        } else {
            last - 1
        },
    ]
}

/// Whether the leaf cells `[lo, hi)` fill any cell of the grid's zoom.
pub fn fills_any(cell_zoom: Zoom, run: [i32; 2]) -> bool {
    let [first, last] = fill_range(cell_zoom, run);
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

/// Where a pier stands under a crossing. None stands under a deck `PIER_MIN_RISE_M` or less
/// above the ground, on a way at grade, or over a deck `LOWER_DECK_GAP_M` or more under its
/// own (Arnis's `place_pillar`, `support_blocked`), and none where the terrain gives no
/// surface to stand on; water stops none. The run is empty where none stands.
fn pier_stands<C: ColumnGrid>(
    cols: &C,
    c: [i32; 2],
    pier: [i32; 2],
    ground: i32,
    per_m: f32,
) -> [i32; 2] {
    let (none, palette_idx) = ([0, 0], cols.palette_idx(c));
    if !value::grounds(palette_idx) || value::transports(palette_idx) {
        return none;
    }
    let surface = pier[1] + slab_depth(per_m);
    if surface - pier[0] <= cells_of_m(PIER_MIN_RISE_M, per_m) {
        return none;
    }
    let gap = cells_of_m(LOWER_DECK_GAP_M, per_m);
    let mut k = 0;
    while k < cols.cnt(c) {
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

/// What a span's value marks as a bore's air, over the palette index of its way.
pub const AIR_OF: u32 = 0x80;

/// What one visit writes in its column, in leaf cells: a deck's slab or an embankment (its
/// way's palette index), or a bore's or a cutting's air (`AIR_OF` over its way's palette
/// index), over `run`; and a pier (`PIER`) over `pier`, which is empty where none stands.
/// `NOTHING` where it writes none.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Write {
    run: [i32; 2],
    val: u32,
    pier: [i32; 2],
}

impl Write {
    const NONE: Write = Write {
        run: [0, 0],
        val: NOTHING,
        pier: [0, 0],
    };
}

/// What the visit from the `k`-th segment over column `c` writes, and where the next visit
/// starts. A visit whose palette index grounds writes what it occupies over the column's face
/// `face`, where `per_m` is the leaf cells a meter, and its pier where one stands.
fn writes<C: ColumnGrid>(cols: &C, c: [i32; 2], k: u32, (face, per_m): (i32, f32)) -> (Write, u32) {
    let (x, next) = visit(cols, c, k);
    if !x.is() || !value::grounds(x.kind) {
        return (Write::NONE, next);
    }
    let o = occupies(&x, face, per_m);
    let w = match o.what {
        Occupy::Under => Write {
            run: o.span,
            val: AIR_OF | x.kind,
            pier: [0, 0],
        },
        Occupy::Over => Write {
            run: o.span,
            val: x.kind,
            pier: if empty(o.pier) {
                o.pier
            } else {
                pier_stands(cols, c, o.pier, face, per_m)
            },
        },
        // An approach's palette index is laid into the cells by the bake (pxl-spec bake/way.rs
        // `lay`), so the way writes nothing here.
        Occupy::Grade => Write::NONE,
    };
    (w, next)
}

/// The most spans one column's visits make (`column`), which is the most the format-12 set
/// makes. A tile past it fails its bake (pxl-spec bake/tile/terrain.rs `fill_fit`).
pub const SPANS: usize = 10;

/// A column's spans, in leaf cells, in the order its visits come, `n` of them: each a deck's
/// slab or an embankment (its way's palette index), a pier (`PIER`), or a bore's or a
/// cutting's air (`AIR_OF` over its way's palette index). With them, its face cell
/// (`ground::face_cell`), under which a bore's air beside the column lines it (`way_cell`); it
/// is `i32::MIN` where the area's edge walls the column or the column is past the area, where
/// nothing lines it. The column's cells are read from these, so its visits are found once: a
/// brick's fill keeps them (pxl-engine fill.rs `spans`, read through `SpanTable`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpanList {
    pub run: [[i32; 2]; SPANS],
    pub val: [u32; SPANS],
    pub n: u32,
    pub face: i32,
}

impl SpanList {
    pub const NONE: SpanList = SpanList {
        run: [[0, 0]; SPANS],
        val: [0; SPANS],
        n: 0,
        face: i32::MIN,
    };

    /// Span `v` over `run`, where there is room; false where there is none.
    fn push(&mut self, run: [i32; 2], v: u32) -> bool {
        if self.n as usize >= SPANS {
            return false;
        }
        self.run[self.n as usize] = run;
        self.val[self.n as usize] = v;
        self.n += 1;
        true
    }
}

/// Column `c`'s spans: what each visit of a line writes there that fills a cell at the grid's
/// zoom (`fills_any`), since `at` reads no other; its face cell; and whether they all fit
/// `SPANS`. A column of the bedrock, past the area or walled by its edge
/// (`ColumnGrid::is_walled`), has no spans and no face, because bedrock wins over a way as it
/// does over the ground's face.
pub fn column<C: ColumnGrid>(cols: &C, c: [i32; 2]) -> (SpanList, bool) {
    let (mut spans, mut fits) = (SpanList::NONE, true);
    if cols.palette_idx(c) == ground::UNKNOWN || cols.is_walled(c) {
        return (spans, fits);
    }
    // The column's face and the tile's scale, read once for all its visits.
    let cell_zoom = cols.cell_zoom();
    let ground = (cols.face(c), cols.per_m());
    spans.face = ground::face_cell(cell_zoom, ground.0);
    let mut k = 0;
    while k < cols.cnt(c) {
        let (w, next) = writes(cols, c, k, ground);
        k = next;
        if w.val != NOTHING && fills_any(cell_zoom, w.run) {
            fits &= spans.push(w.run, w.val);
        }
        if w.val != NOTHING && fills_any(cell_zoom, w.pier) {
            fits &= spans.push(w.pier, PIER);
        }
    }
    (spans, fits)
}

/// The spans of a brick's columns and of the ring around it, as its fill keeps them
/// (pxl-engine fill.rs `spans`), read in place: column `c`'s face cell and how many spans it
/// has (`SpanList`), and its `j`-th span with its value.
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
/// it and the row over it. This is the order of the grid's cells, in which a tree's leaves
/// read them.
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

/// What the ways write in cell `z` of column `c` at zoom `cell_zoom`, where its spans and
/// those beside it are kept in `k` (`at` of its own). A deck's slab or its pier comes first,
/// or else a bore's air (`AIR`). Under its face cell, where a cave may take it
/// (`ground::face_cell`), it is otherwise the way's palette index wherever a bore's air lies
/// beside it, under or over it or across one of its four faces, the first in `near`'s order;
/// so a cave never opens into a way, whatever carves the ground. Otherwise it is `NOTHING`.
/// It is read from the spans in place, the column's own first, and a cell the column writes
/// reads nothing beside it.
pub fn way_cell(k: &impl SpanTable, c: [i32; 2], cell_zoom: Zoom, z: i32) -> u32 {
    let (face, n) = k.header(c);
    let v = at(k, c, n, cell_zoom, z);
    if v != NOTHING {
        return if v & AIR_OF != 0 { AIR } else { v };
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
            // Air read at `z + dz` lines `z`, under the face cell, where no slab or pier of its column
            // fills it.
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
    use crate::ground;

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
            let face = ground::face_cell(
                cell_zoom,
                ground::face_height(cell_zoom, ground::cells(cell_zoom, top as f32)),
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
                lined_hull(&k, [0, 0], cell_zoom, top),
                hull,
                "{:?} at {cell_zoom}, top {top}",
                k.0
            );
        }
    }
}
