// pxl-spec/way.rs

//! The ways off the ground, as a column makes them: a bridge's deck and its
//! piers, a tunnel's bore and the way's ground around it, and an approach's embankment or
//! cutting. A brick's fill makes each column's spans once, on the CPU, and keeps them
//! (`column`); the query, on the CPU and on the GPU, reads a cell off those spans (pxl-engine
//! way.rs `way_cell`, clipmap.rs `way_at`, pxl-render way.wgsl). The bake makes them by the
//! same rule to know which lines a zoom draws (bake/tile/terrain.rs `drawn`). The lines
//! themselves are solved offline (bake/separation.rs).
//!
//! A cell is asked of the lines over its column, each visit of a line being its own crossing:
//! the run of the line's segments, one after another, that lie over the column. A column's
//! visits make its spans once (`column`), and what a cell of the column holds is read from
//! them (pxl-engine way.rs `at`). Ground beside the air the spans leave takes that way's
//! palette index, read from the spans of the columns beside it (pxl-engine `way_cell`), so a
//! brick's fill and a reader read its ring alike. Whatever is under half a cell in any
//! dimension at a zoom is absent at that zoom, as the bake drops a line under half a cell
//! wide: a slab or a bore fills a cell where it holds half the cell's height or more, and a
//! pier, one leaf cell across, stands only in columns no more than two leaf cells wide.
//!
//! The code uses index loops and no `Option` (a missing crossing is `Crossing::NONE`), and
//! words for tags. Where a column is crossed, its road's heights and its pier (`on_way`,
//! `crossing`, `pier_covers`) are exact, in integers over the wire's own (points in eighths of
//! a cell, heights and stations in leaf cells), so the CPU and a GPU decide them alike whatever
//! they fuse or reorder. There is no division but a rational's
//! floor, and the one root, a segment's direction, is rounded once by the CPU and handed to a
//! GPU in a word (`Segment::dir`).

use crate::asset::cube::{Ground, Sentinel};
use crate::ground;
use crate::param::way::BEAM_PILLAR_INTERVAL_M;
use crate::param::way::DECK_THICKNESS_M;
use crate::param::way::LINE_PAST;
use crate::param::way::PIER_MIN_RISE_M;
use crate::param::way::STACKED_DECK_HEADROOM_M;
use crate::param::way::WIDE_BEAM_PIER_INTERVAL_M;
use crate::space::coord::adapter::epsg3857::cells_of_m;
use crate::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM};
use crate::value;

/// Points are counted in eighths of the tile's cell.
pub const POINT_SCALE: f32 = 8.0;

// From louis-e/arnis `c96872e`, Copyright (c) 2022-2026 Louis Erbkamm, under
// the Apache License, Version 2.0 (THIRD_PARTY_NOTICES.md); changed to stand
// on a deck's line rather than place Minecraft blocks.

/// A deck at least this far under a pier blocks it: one cell short of the stacked headroom
/// (Arnis's `LOWER_DECK_GAP`).
pub const LOWER_DECK_GAP_M: f32 = STACKED_DECK_HEADROOM_M - 1.0;

/// A line's form, as its byte on the wire.
pub const BRIDGE: u32 = 0;
pub const TUNNEL: u32 = 1;
pub const APPROACH: u32 = 2;

/// What a span or a cell holds where the ways write nothing there (pxl-engine `way_cell`).
pub const NOTHING: u32 = u32::MAX;

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
/// from `a` to `b`, its road surface `heights` there in leaf cells, its width,
/// the palette index it shows, its form, its tunnel's clear height in leaf
/// cells, whether it is its line's first or last, where the line ends
/// square, and whether the one stored before it is its line's one before
/// it; and its direction (`dir`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: [i32; 2],
    pub b: [i32; 2],
    pub heights: [i32; 2],
    pub width: u32,
    pub asset_id: u32,
    pub form: u32,
    pub clearance: u32,
    pub first: u32,
    pub last: u32,
    pub continues: u32,
    pub dir: [i32; 2],
}

/// A segment's words: `a` and `b` each two `i16` eighths of a cell, the heights
/// two `i16`, then width (eighths), asset_id, form with first, last and
/// continues over it, and clearance, a byte each; then its direction, two `i16`,
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
            heights: i16_pair(w[2]),
            width: byte(0),
            asset_id: byte(1),
            form: byte(2) & 3,
            first: (byte(2) >> 2) & 1,
            last: (byte(2) >> 3) & 1,
            continues: (byte(2) >> 4) & 1,
            clearance: byte(3),
            dir: i16_pair(w[SEGMENT_DIR]),
        }
    }

    /// Its words, from the wire's integers: its two points in eighths, its heights in leaf cells,
    /// and its bytes (width in eighths, asset_id, `flags`, clearance); then its run's direction (`dir`).
    pub fn words(
        [a, b]: [[i16; 2]; 2],
        heights: [i16; 2],
        [width, asset_id, flags, clearance]: [u8; 4],
    ) -> [u32; SEGMENT_WORDS] {
        let d = dir([0, 1].map(|k| i32::from(b[k]) - i32::from(a[k])));
        [
            pair_word(a[0], a[1]),
            pair_word(b[0], b[1]),
            pair_word(heights[0], heights[1]),
            u32::from(width)
                | u32::from(asset_id) << 8
                | u32::from(flags) << 16
                | u32::from(clearance) << 24,
            pair_word(d[0] as i16, d[1] as i16),
        ]
    }
}

/// Where a tile's piers may stand, in leaf cells: the bents' lattice along each axis, with a
/// step of `step[w]` for the narrow beam (`w` 0) and the wide (1), and where the tile's
/// corner lies inside a step (`phase[w][axis]`), so a station is the world's and not the
/// tile's; and the leaf cells a tile's cell spans a side (`leaf`), in which a pier's section
/// is measured. All are whole leaf cells, as every length given in meters is (`cells_of_m`),
/// so a station is exact. It is made once a tile (procedural.rs `Frame::stations`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StationGrid {
    pub step: [i32; 2],
    pub phase: [[i32; 2]; 2],
    pub leaf: u32,
}

/// A bent's spacing for the narrow beam (`w` 0) and the wide (1), in meters.
pub const INTERVAL_M: [f32; 2] = [BEAM_PILLAR_INTERVAL_M, WIDE_BEAM_PIER_INTERVAL_M];

/// What a brick's lines open with (brick.rs `LINES_*`): its tile's leaf cells a
/// ground meter (`Frame::per_m`), as a float's bits, then its stations
/// (`StationGrid`), a word each: the narrow beam's step and then the wide's, the phase `phase[w][axis]` at
/// `HDR_PHASE + 2w + axis`, and the leaf cells a cell.
pub const HDR_PER_M: usize = 0;
pub const HDR_STEP: usize = 1;
pub const HDR_PHASE: usize = 3;
pub const HDR_LEAF: usize = 7;
pub const HDR_WORDS: u32 = 8;

pub fn hdr(per_m: f32, st: &StationGrid) -> [u32; HDR_WORDS as usize] {
    let [[p0, p1], [p2, p3]] = st.phase;
    let mut w = [0; HDR_WORDS as usize];
    w[HDR_PER_M] = per_m.to_bits();
    w[HDR_STEP] = st.step[0] as u32;
    w[HDR_STEP + 1] = st.step[1] as u32;
    w[HDR_PHASE] = p0 as u32;
    w[HDR_PHASE + 1] = p1 as u32;
    w[HDR_PHASE + 2] = p2 as u32;
    w[HDR_PHASE + 3] = p3 as u32;
    w[HDR_LEAF] = st.leaf;
    w
}

pub(crate) fn of_hdr(w: [u32; HDR_WORDS as usize]) -> (f32, StationGrid) {
    let i = |k: usize| w[k] as i32;
    let st = StationGrid {
        step: [i(HDR_STEP), i(HDR_STEP + 1)],
        phase: [
            [i(HDR_PHASE), i(HDR_PHASE + 1)],
            [i(HDR_PHASE + 2), i(HDR_PHASE + 3)],
        ],
        leaf: w[HDR_LEAF],
    };
    (f32::from_bits(w[HDR_PER_M]), st)
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
    pub asset_id: u32,
    pub form: u32,
    pub pier: u32,
    pub clearance: u32,
}

impl Crossing {
    /// No crossing: one of asset_id `NOTHING`.
    pub const NONE: Crossing = Crossing {
        low: 0,
        top: 0,
        asset_id: NOTHING,
        form: 0,
        pier: 0,
        clearance: 0,
    };

    /// Whether it is one (`NONE` is not).
    pub fn is(&self) -> bool {
        self.asset_id != NOTHING
    }
}

/// The road surface of `s` at the share `n / len2` of it, clamped to its ends, in leaf cells
/// rounded half to even: `heights[0] + Δ · n / len2`, whose numerator is under 2^51 and whose
/// quotient lies within the rise `Δ`, under 2^17, the heights each an `i16` (`nearest`).
fn surface(s: &Segment, n: i64, len2: i64) -> i32 {
    let rise = i64::from(s.heights[1] - s.heights[0]);
    nearest(rise * n.clamp(0, len2), len2, s.heights[0])
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
        asset_id: s.asset_id,
        form: s.form,
        pier: u32::from(s.form != TUNNEL && pier_covers(st, s, [n, len2], at)),
        clearance: s.clearance,
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
            span: [low, top + c.clearance as i32],
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

/// The columns a brick's fill reads its ways from (`brick::Brick`): each column's segments,
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
    fn asset_id(&self, c: [i32; 2]) -> u32;
    /// Whether the column is the service area's wall (`area::is_wall`), where no way writes, as
    /// none does past it.
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
    let (none, asset_id) = ([0, 0], cols.asset_id(c));
    if !value::is_ground(asset_id) || value::is_transport(asset_id) {
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
/// index), over `run`; and a pier (`Ground::Building`) over `pier`, which is empty where none
/// stands. `NOTHING` where it writes none.
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
    if !x.is() || !value::is_ground(x.asset_id) {
        return (Write::NONE, next);
    }
    let o = occupies(&x, face, per_m);
    let w = match o.what {
        Occupy::Under => Write {
            run: o.span,
            val: AIR_OF | x.asset_id,
            pier: [0, 0],
        },
        Occupy::Over => Write {
            run: o.span,
            val: x.asset_id,
            pier: if empty(o.pier) {
                o.pier
            } else {
                pier_stands(cols, c, o.pier, face, per_m)
            },
        },
        // An approach's palette index is laid into the cells by the bake (bake/way.rs
        // `lay`), so the way writes nothing here.
        Occupy::Grade => Write::NONE,
    };
    (w, next)
}

/// The most spans a column keeps (`column`): one past it is no column a brick holds, and the
/// brick is refused for it and reads the coarser clip level (`SpanList::past`).
pub const SPAN_CAP: usize = 10;

/// A column's spans, in leaf cells, in the order its visits come, `n` of them: each a deck's
/// slab or an embankment (its way's palette index), a pier (`Ground::Building`), or a bore's or a
/// cutting's air (`AIR_OF` over its way's palette index). With them, its face cell
/// (`ground::face_cell`), under which a bore's air beside the column lines it (pxl-engine
/// `way_cell`); it is `i32::MIN` where the area's edge walls the column or the column is past
/// the area, where nothing lines it. `past` says a visit made more spans than `SPAN_CAP`, so these
/// are not all the column's. The column's cells are read from these, so its visits are found
/// once: a brick's fill keeps them (pxl-engine fill.rs `spans`, read through `way::SpanTable`
/// there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpanList {
    pub run: [[i32; 2]; SPAN_CAP],
    pub val: [u32; SPAN_CAP],
    pub n: u32,
    pub face: i32,
    pub past: bool,
}

impl SpanList {
    pub const NONE: SpanList = SpanList {
        run: [[0, 0]; SPAN_CAP],
        val: [0; SPAN_CAP],
        n: 0,
        face: i32::MIN,
        past: false,
    };

    /// Span `v` over `run`, where there is room; a span past `SPAN_CAP` marks the list `past`.
    fn push(&mut self, run: [i32; 2], v: u32) {
        if (self.n as usize) < SPAN_CAP {
            self.run[self.n as usize] = run;
            self.val[self.n as usize] = v;
            self.n += 1;
        } else {
            self.past = true;
        }
    }
}

/// Column `c`'s spans: what each visit of a line writes there that fills a cell at the grid's
/// zoom (`fills_any`), since pxl-engine's `at` reads no other, and its face cell.
/// A column of the bedrock, past the area or walled by its edge (`ColumnGrid::is_walled`), has
/// no spans and no face, because bedrock wins over a way as it does over the ground's face.
pub fn column<C: ColumnGrid>(cols: &C, c: [i32; 2]) -> SpanList {
    let mut spans = SpanList::NONE;
    if cols.asset_id(c) == Sentinel::Unknown as u32 || cols.is_walled(c) {
        return spans;
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
            spans.push(w.run, w.val);
        }
        if w.val != NOTHING && fills_any(cell_zoom, w.pier) {
            spans.push(w.pier, Ground::Building as u32);
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A column past `SPAN_CAP` says so, and a brick that holds it is refused rather than drawn
    /// with spans dropped (pxl-core `Fill::Refused`).
    #[test]
    fn past_spans_says_so() {
        let mut s = SpanList::NONE;
        for k in 0..SPAN_CAP as i32 {
            s.push([k, k + 1], 1);
        }
        assert!(!s.past);
        s.push([20, 21], 1);
        assert!(s.past);
        assert_eq!(
            s.n as usize, SPAN_CAP,
            "the first ones kept, the list marked"
        );
    }
}
