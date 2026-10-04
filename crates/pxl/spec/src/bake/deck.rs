// pxl-spec/bake/deck.rs

//! A page's decks: the ways it carries above its ground — bridges,
//! viaducts, the ramps of an interchange.
//!
//! The ground is a heightfield and a cell of it holds one way, flat on the
//! surface. A deck is neither: it stands off the ground, crosses other ways
//! and other decks, and has a height of its own along its length. So it is
//! kept in its own shape, a line, and drawn into the chunk's columns when the
//! chunk is built (issue/20261001-ways-off-the-ground.md). Decks are sparse —
//! most pages carry none — and a line is a few bytes a point where a lane
//! would be a byte a cell on every page.
//!
//! A deck is a polyline in its page's cells, a top at each point, a width and
//! the transport type its surface shows; a point may stand on a pier. Where a
//! deck lies is the page's lattice, the ground's (space/pyramid.rs
//! `GROUND`), so a level's page holds its decks in its own cells as it holds
//! its ground. A top is in z16 cells, as the elevation is, at every level.
//!
//! The heights are solved offline over the whole network, structure by
//! structure (pxl-geo-bake deck/); nothing here decides one.

use std::borrow::Cow;

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, BranchViewImpl, NodeView};

use super::{malformed, way};
use crate::procedural::{pier_covers, Frame};
use crate::space::hierarchy::child_of;
use crate::space::pyramid::{GROUND, PAGE_CELLS};

/// A point's coordinate in eighths of its page's cell, from the page's
/// south-west corner: an `i16` reaches four thousand cells either way, far
/// past a page's 512, so a deck's points outside its page are held as
/// exactly as those inside it.
pub const POINT_SCALE: f64 = 8.0;

/// A deck's slab, top to underside, in z16 cells. One cell is about the
/// two metres a road deck's structure is.
pub const THICKNESS: i32 = 1;

/// The widest a way is drawn, in its page's cells: what a deck's width byte
/// holds. The ground draws a way no wider, so it is one width on and off a
/// bridge, and how far past a page anything drawn on it can come from is bounded.
pub const WIDEST: f64 = u8::MAX as f64 / POINT_SCALE;

// ---- in ground metres, where the source gives none ----

// clear under a deck
pub const ROAD_CLEARANCE_M: f64 = 4.5; // over a road: UNECE AGR Annex II III.4

// Arnis's deck heights over a path or a track, less the slab under them
pub const PATH_CLEARANCE_M: f64 = 5.0 - DECK_THICKNESS_M; // a footway, path or cycleway
pub const RAIL_CLEARANCE_M: f64 = 8.0 - DECK_THICKNESS_M; // a railway: its catenary, 6 over the bed
pub const DECK_THICKNESS_M: f64 = 2.0; // a deck's slab under its surface: Arnis's structure depth

// clear inside a bore
pub const ROAD_HEADROOM_M: f64 = ROAD_CLEARANCE_M; // AGR's overhead clearance holds in a tunnel too
pub const PATH_HEADROOM_M: f64 = 2.5; // pixelet's, no source found
pub const RAIL_HEADROOM_M: f64 = 8.0; // pixelet's, no source found
pub const BORE_COVER_M: f64 = 3.0; // a bore's shell over its headroom: pixelet's, no source found

/// The steepest a way under or through a structure is taken to climb, so a
/// clearance holds across the road on any of them.
pub const MAX_GRADE: f64 = 0.30;

/// Whole cells clear for `m` metres across a road, `cell_per_m` cells a metre,
/// where it steps `step` cells to the next column: up to `MAX_GRADE` the clear
/// height stands higher by the grade's secant, a least rounds up, and a
/// staircase's step, a cell at least, is taken out of it at the step's edge.
pub fn clear_cells(m: f64, cell_per_m: f64, step: i32) -> i32 {
    libm::ceil(m * libm::hypot(1.0, MAX_GRADE) * cell_per_m) as i32 + step.max(1)
}

/// What a way of transport type `kind` needs clear under a deck over it.
pub fn clearance_m(kind: u8) -> f64 {
    by_kind(kind, [ROAD_CLEARANCE_M, PATH_CLEARANCE_M, RAIL_CLEARANCE_M])
}

/// What a way of transport type `kind` needs clear over it in its bore.
pub fn headroom_m(kind: u8) -> f64 {
    by_kind(kind, [ROAD_HEADROOM_M, PATH_HEADROOM_M, RAIL_HEADROOM_M])
}

fn by_kind(kind: u8, [road, path, rail]: [f64; 3]) -> f64 {
    use crate::cell::cube::cube_types::transport as t;
    match kind {
        k if k == t::rail_main || k == t::rail_subway => rail,
        k if k == t::pedestrian => path,
        _ => road,
    }
}

/// What a line is, from the source's flags alone; the byte is the wire's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Form {
    Deck = 0,  // a bridge: a slab under its surface
    Bore = 1,  // a tunnel: its clear height and shell over its surface
    Grade = 2, // a way on the ground taking a structure's end to it: a cutting or an embankment
}

impl Form {
    const ALL: [Form; 3] = [Form::Deck, Form::Bore, Form::Grade];

    /// A byte `read_branch` let through.
    pub fn of(byte: u8) -> Form {
        Form::ALL[usize::from(byte)]
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Can stand below the ground.
    pub fn sinks(self) -> bool {
        self != Form::Deck
    }

    /// Can stand above the ground.
    pub fn rises(self) -> bool {
        self != Form::Bore
    }
}

/// A resident record's room for decks and their points, about 43 KiB, chosen
/// from what a record can spend, not from the data; each lane whole cache lines.
pub const MAX_DECKS: usize = 1023;
pub const MAX_POINTS: usize = 6144;

/// A page's decks, as structure-of-arrays: deck `d` is points
/// `starts[d]..starts[d + 1]`, so `starts` is one longer than the decks and
/// none is `[0]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Decks {
    pub starts: Cow<'static, [u32]>,
    /// (x, y) in eighths of the page's cell (`POINT_SCALE`).
    pub points: Cow<'static, [[i16; 2]]>,
    /// The deck's top at each point, in z16 cells.
    pub tops: Cow<'static, [i16]>,
    /// 1 where a pier stands under the point.
    pub piers: Cow<'static, [u8]>,
    /// A deck's width, in eighths of the page's cell.
    pub width: Cow<'static, [u8]>,
    /// The transport type a deck's surface shows, as the catalog numbers it
    /// (cell/cube_type.spec.rs `transport`).
    pub kind: Cow<'static, [u8]>,
}

impl Default for Decks {
    fn default() -> Self {
        Builder::default().finish()
    }
}

/// One deck of a page, read where it lies.
#[derive(Clone, Copy, Debug)]
pub struct Deck<'a> {
    pub points: &'a [[i16; 2]],
    pub tops: &'a [i16],
    pub piers: &'a [u8],
    pub width: u8,
    pub kind: u8,
}

/// A line over one column: its road surface in z16 cells, how far that
/// changes across one of its page's cells, and whether its pier's section
/// covers the column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub top: f64,
    pub rise: f64,
    pub kind: u8,
    pub pier: bool,
}

/// What a line occupies of one column, in z16 cells, each `[lo, hi)`; the
/// paint and the query both use it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Occupies {
    /// The way on the ground; the column's top cell is its surface.
    Grade,
    /// A slab under the surface, and a pier
    /// from the ground to the slab where its section covers the column.
    Above { slab: [i32; 2], pier: Option<[i32; 2]> },
    /// Air from the surface up; the cell under it is the way's.
    Through { air: [i32; 2] },
}

impl Over {
    /// `cell_per_m`: z16 cells a ground metre on the column's page.
    pub fn occupies(&self, ground: i32, cell_per_m: f64) -> Occupies {
        let top = round(self.top) as i32;
        // a slab overlaps the next column's by a cell however far the surface steps
        let step = libm::ceil(self.rise.abs()) as i32;
        let thick = (round(DECK_THICKNESS_M * cell_per_m) as i32).max(step + 1);
        let slab = |floor: i32| {
            let lo = (top - thick).max(floor);
            Occupies::Above {
                slab: [lo, top],
                pier: (self.pier && lo > ground).then_some([ground, lo]),
            }
        };
        // a deck or a bore is one whatever its height: the solve holds it off the ground
        match self.form {
            Form::Deck => slab(i32::MIN),
            Form::Bore => Occupies::Through {
                air: [top, top + clear_cells(headroom_m(self.kind), cell_per_m, step)],
            },
            // a road off the ground past a structure stands on piers as a deck does, and is
            // open to the sky where it is cut in
            Form::Grade if top > ground => slab(ground),
            Form::Grade if top < ground => Occupies::Through { air: [top, ground] },
            Form::Grade => Occupies::Grade,
        }
    }
}

impl Decks {
    pub fn len(&self) -> usize {
        self.width.len()
    }

    pub fn is_empty(&self) -> bool {
        self.width.is_empty()
    }

    pub fn deck(&self, d: usize) -> Deck<'_> {
        let (a, b) = (self.starts[d] as usize, self.starts[d + 1] as usize);
        Deck {
            points: &self.points[a..b],
            tops: &self.tops[a..b],
            piers: &self.piers[a..b],
            width: self.width[d],
            kind: self.kind[d],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Deck<'_>> {
        (0..self.len()).map(|d| self.deck(d))
    }

    /// Every deck over each column of the `[x0, x1) × [y0, y1)` cells of the
    /// page, as `each(x, y, deck, over)`. A column is under a deck where its
    /// center lies within half the deck's width of the deck's line; a column
    /// under two of a deck's segments, at a joint, is told of both, and the
    /// caller keeps the higher.
    pub fn over_cells(&self, [x0, x1]: [i64; 2], [y0, y1]: [i64; 2], mut each: impl FnMut(i64, i64, usize, Over)) {
        for (d, deck) in self.iter().enumerate() {
            let half = deck.half_width();
            let [lo, hi] = deck.bounds();
            if hi[0] + half < x0 as f64 || lo[0] - half > x1 as f64 || hi[1] + half < y0 as f64 || lo[1] - half > y1 as f64 {
                continue;
            }
            for s in 0..deck.points.len().saturating_sub(1) {
                let (a, b) = (deck.point(s), deck.point(s + 1));
                let cells = |u: f64, v: f64, lo: i64, hi: i64| ((u.min(v) - half).floor() as i64).max(lo)..((u.max(v) + half).ceil() as i64).min(hi);
                for y in cells(a[1], b[1], y0, y1) {
                    for x in cells(a[0], b[0], x0, x1) {
                        if let Some(over) = deck.over_segment(s, [x as f64 + 0.5, y as f64 + 0.5]) {
                            each(x, y, d, over);
                        }
                    }
                }
            }
        }
    }

    /// Every deck over the point `at`, in the page's cells.
    pub fn over_point(&self, at: [f64; 2]) -> impl Iterator<Item = Over> + '_ {
        self.iter().flat_map(move |deck| (0..deck.points.len().saturating_sub(1)).filter_map(move |s| deck.over_segment(s, at)))
    }

    /// The lowest surface of a line that can sink inside the box, conservatively.
    pub fn lowest_in(&self, x: [f64; 2], y: [f64; 2]) -> Option<i16> {
        self.segments_in(Form::sinks, x, y).map(|[lo, _]| lo).min()
    }

    /// The highest surface of a line that can rise inside the box,
    /// conservatively: its highest point counts wherever its footprint reaches the box.
    pub fn highest_in(&self, x: [f64; 2], y: [f64; 2]) -> Option<i16> {
        self.segments_in(Form::rises, x, y).map(|[_, hi]| hi).max()
    }

    /// The lower and higher end of each segment whose footprint reaches the
    /// box, of the lines whose form `keeps`.
    fn segments_in(&self, keeps: fn(Form) -> bool, [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> impl Iterator<Item = [i16; 2]> + '_ {
        self.iter().filter(move |d| keeps(d.form)).flat_map(move |deck| {
            let half = deck.footprint();
            (0..deck.points.len().saturating_sub(1)).filter_map(move |s| {
                let (a, b) = (deck.point(s), deck.point(s + 1));
                let apart = a[0].max(b[0]) + half < x0 || a[0].min(b[0]) - half > x1 || a[1].max(b[1]) + half < y0 || a[1].min(b[1]) - half > y1;
                let (ta, tb) = (deck.tops[s], deck.tops[s + 1]);
                (!apart).then_some([ta.min(tb), ta.max(tb)])
            })
        })
    }
}

impl Deck<'_> {
    /// Point `i` in the page's cells.
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.points[i].map(|v| f64::from(v) / POINT_SCALE)
    }

    pub fn half_width(&self) -> f64 {
        f64::from(self.width) / POINT_SCALE / 2.0
    }

    /// The corners of the box its line lies in, in the page's cells.
    pub fn bounds(&self) -> [[f64; 2]; 2] {
        let mut b = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
        for i in 0..self.points.len() {
            let p = self.point(i);
            for k in 0..2 {
                (b[0][k], b[1][k]) = (b[0][k].min(p[k]), b[1][k].max(p[k]));
            }
        }
        b
    }

    /// What segment `s` is over `at`: the top at the nearest point of the
    /// segment, if `at` is within half the width of it. A pier stands at a
    /// point and takes the cell its point is in, at least, and half the
    /// deck's width across.
    fn over_segment(&self, s: usize, at: [f64; 2]) -> Option<Over> {
        let (a, b) = (self.point(s), self.point(s + 1));
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = along(a, [dx, dy], at);
        // a line ends square where it begins and ends, as the ground cuts a way, so the
        // next line and the paint start where it stops; its joints stay round
        if (s == 0 && t < 0.0) || (s + 2 == self.points.len() && t > 1.0) {
            return None;
        }
        let t = t.clamp(0.0, 1.0);
        let near = [a[0] + dx * t, a[1] + dy * t];
        let half = self.half_width();
        if (at[0] - near[0]).hypot(at[1] - near[1]) > half {
            return None;
        }
        let rise = f64::from(self.tops[s + 1]) - f64::from(self.tops[s]);
        Some(Over {
            top: f64::from(self.tops[s]) + rise * t,
            rise: if len2 > 0.0 { rise / libm::sqrt(len2) } else { 0.0 },
            kind: self.kind,
            pier: on(s) || on(s + 1),
        })
    }
}

/// Where the nearest point of the line from `a` along `d` to `p` lies, as a
/// share of `d`; 0 when `d` is none.
fn along(a: [f64; 2], d: [f64; 2], p: [f64; 2]) -> f64 {
    let len2 = d[0] * d[0] + d[1] * d[1];
    if len2 > 0.0 { ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2 } else { 0.0 }
}

pub fn footprint(width: f64) -> f64 {
    width / 2.0 + LINE_PAST
}

/// Decks one level up: half a cell wide or more, simplified to half a cell,
/// widest first within a record.
pub fn fold_decks(children: [&Decks; 4], level: u32) -> Decks {
    let mut out = Builder::default();
    let n = f64::from(PAGE_CELLS);
    // The parent's half cell of height, in z16 cells.
    let height_tolerance = f64::from(GROUND.finest_in(level)) / 2.0;
    for (q, child) in children.iter().enumerate() {
        let [qx, qy] = child_of([0, 0], q).map(|v| v as f64 * n);
        for deck in child.iter() {
            let width = deck.width / 2;
            if f64::from(width) < POINT_SCALE / 2.0 {
                continue;
            }
            let points: Vec<[f64; 2]> = (0..deck.points.len()).map(|i| deck.point(i)).map(|[x, y]| [(x + qx) / 2.0, (y + qy) / 2.0]).collect();
            let keep = simplify(&points, deck.tops, 0.5, height_tolerance);
            let kept: Vec<usize> = (0..points.len()).filter(|&i| keep[i] || deck.piers[i] != 0).collect();
            out.push(kept.iter().map(|&i| (points[i], deck.tops[i], deck.piers[i])), width, deck.kind);
        }
    }
    out.finish()
}

/// Which points of a line Douglas–Peucker keeps at `across` cells and
/// `height` z16 cells: the ends, and every point the line without it would
/// miss by more than either.
fn simplify(points: &[[f64; 2]], tops: &[i16], across: f64, height: f64) -> Vec<bool> {
    let mut keep = vec![false; points.len()];
    if points.is_empty() {
        return keep;
    }
    let last = points.len() - 1;
    (keep[0], keep[last]) = (true, true);
    let mut stack = vec![(0, last)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (points[a], points[b]);
        let (dx, dy) = (pb[0] - pa[0], pb[1] - pa[1]);
        let mut worst = (0.0, a);
        for i in a + 1..b {
            let p = points[i];
            let t = along(pa, [dx, dy], p).clamp(0.0, 1.0);
            let off = libm::hypot(p[0] - pa[0] - dx * t, p[1] - pa[1] - dy * t) / across;
            let rise = (f64::from(tops[i]) - (f64::from(tops[a]) + (f64::from(tops[b]) - f64::from(tops[a])) * t)).abs() / height;
            let e = off.max(rise);
            if e > worst.0 {
                worst = (e, i);
            }
        }
        if worst.0 > 1.0 {
            keep[worst.1] = true;
            stack.push((a, worst.1));
            stack.push((worst.1, b));
        }
    }
    keep
}

/// A page's decks, written one at a time.
#[derive(Default)]
pub struct Builder {
    starts: Vec<u32>,
    points: Vec<[i16; 2]>,
    tops: Vec<i16>,
    piers: Vec<u8>,
    width: Vec<u8>,
    kind: Vec<u8>,
}

impl Builder {
    /// A deck from its points in the page's cells, each with its top and
    /// whether a pier stands under it. A line of fewer than two points, or
    /// of no width, is no deck.
    pub fn push(&mut self, points: impl IntoIterator<Item = ([f64; 2], i16, u8)>, width: u8, kind: u8) {
        let at = self.points.len();
        for ([x, y], top, pier) in points {
            let q = |v: f64| (v * POINT_SCALE).round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            self.points.push([q(x), q(y)]);
            self.tops.push(top);
            self.piers.push(pier);
        }
        if self.points.len() - at < 2 || width == 0 {
            self.points.truncate(at);
            self.tops.truncate(at);
            self.piers.truncate(at);
            return;
        }
        self.starts.push(at as u32);
        self.width.push(width);
        self.kind.push(kind);
    }

    /// Whether the decks so far fit a record.
    pub fn fits(&self) -> bool {
        self.width.len() <= MAX_DECKS && self.points.len() <= MAX_POINTS
    }

    pub fn finish(mut self) -> Decks {
        self.starts.push(self.points.len() as u32);
        Decks {
            starts: self.starts.into(),
            points: self.points.into(),
            tops: self.tops.into(),
            piers: self.piers.into(),
            width: self.width.into(),
            kind: self.kind.into(),
        }
    }
}

// ---- The wire ----

// decks = [starts, points, tops, piers, width, kind], the points flattened.

/// A page's decks as its packet's branch.
pub fn write_branch<'a>(decks: &'a Decks, b: &mut BranchViewImpl<'a>) {
    b.branch(Some(&mut |d| {
        d.u32_array(Some(&decks.starts))
            .i16_array(Some(decks.points.as_flattened()))
            .i16_array(Some(&decks.tops))
            .u8_array(Some(&decks.piers))
            .u8_array(Some(&decks.width))
            .u8_array(Some(&decks.kind));
    }));
}

/// A packet's decks, where they lie.
pub struct Lanes<'a> {
    starts: &'a [u32],
    points: &'a [[i16; 2]],
    tops: &'a [i16],
    piers: &'a [u8],
    width: &'a [u8],
    kind: &'a [u8],
}

/// The branch's lanes: refused unless they are one page's decks — as many
/// widths and kinds as decks, as many tops and piers as points, the starts
/// rising from the first point to past the last, and no more than a record
/// holds.
pub fn read_branch(d: BranchCursor<'_>) -> Result<Lanes<'_>, MarshalError> {
    let starts = d.at(0)?.get_array::<u32>()?;
    let points = match d.at(1)?.get_array::<i16>()?.as_chunks::<2>() {
        (points, []) => points,
        _ => return Err(malformed()),
    };
    let lanes = Lanes {
        starts,
        points,
        tops: d.at(2)?.get_array::<i16>()?,
        piers: d.at(3)?.get_array::<u8>()?,
        width: d.at(4)?.get_array::<u8>()?,
        kind: d.at(5)?.get_array::<u8>()?,
    };
    let n = lanes.width.len();
    let shaped = lanes.kind.len() == n
        && starts.len() == n + 1
        && lanes.tops.len() == points.len()
        && lanes.piers.len() == points.len()
        && n <= MAX_DECKS
        && points.len() <= MAX_POINTS
        && starts.first() == Some(&0)
        && starts.last() == Some(&(points.len() as u32))
        && starts.windows(2).all(|w| w[0] <= w[1])
        && lanes.form.iter().all(|&f| usize::from(f) < Form::ALL.len());
    shaped.then_some(lanes).ok_or_else(malformed)
}

impl Lanes<'_> {
    /// Copied out, for a page held in hand.
    pub fn to_decks(&self) -> Decks {
        Decks {
            starts: self.starts.to_vec().into(),
            points: self.points.to_vec().into(),
            tops: self.tops.to_vec().into(),
            piers: self.piers.to_vec().into(),
            width: self.width.to_vec().into(),
            kind: self.kind.to_vec().into(),
        }
    }

    /// Copied into a record's room, and read from there.
    pub fn into_room(&self, room: Room) -> Decks {
        fn put<T: Copy>(dst: &'static mut [T], src: &[T]) -> &'static [T] {
            let dst = &mut dst[..src.len()];
            dst.copy_from_slice(src);
            dst
        }
        Decks {
            starts: put(room.starts, self.starts).into(),
            points: put(room.points, self.points).into(),
            tops: put(room.tops, self.tops).into(),
            piers: put(room.piers, self.piers).into(),
            width: put(room.width, self.width).into(),
            kind: put(room.kind, self.kind).into(),
        }
    }
}

/// A record's room for decks, each lane as long as a record holds
/// (`MAX_DECKS`, `MAX_POINTS`).
pub struct Room {
    pub starts: &'static mut [u32],
    pub points: &'static mut [[i16; 2]],
    pub tops: &'static mut [i16],
    pub piers: &'static mut [u8],
    pub width: &'static mut [u8],
    pub kind: &'static mut [u8],
}

#[cfg(test)]
#[path = "deck.test.rs"]
mod test;
