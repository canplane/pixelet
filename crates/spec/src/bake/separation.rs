// pxl-spec/bake/separation.rs

//! A tile's ways off the ground: polylines in the tile's cells with their road
//! surface in leaf cells at each point, solved offline over the whole network.
//! What they look like — slab, piers, bore, embankment, cutting — the client
//! makes from the surface and these numbers.

use std::borrow::Cow;

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::{BranchCursor, BranchViewImpl, NodeView};

use pxl_portable::lattice::CELL_CENTER;
use pxl_portable::ways::{self, Segment, Stations};

use super::{malformed, way};
use crate::procedural::Frame;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::TILE_CELLS;
use xpute_kit::math::scalar::round;

/// Points are in eighths of the tile's cell, which an `i16` holds far past
/// the tile's edge (pxl-portable ways.rs).
pub const POINT_SCALE: f64 = ways::POINT_SCALE as f64;

/// The farthest past half its width the client draws a line, in its tile's
/// cells; the bake sizes a line's tile and extent by it.
pub const LINE_PAST: f64 = ways::LINE_PAST as f64;

/// A line's width byte for `width` of its tile's cells, in eighths rounded
/// half to even, an eighth at least: the widest a way is drawn is what the
/// byte holds. The ground draws a way no wider, so it is one width on and off
/// a bridge, and how far past a tile anything drawn on it can come from is
/// bounded.
pub fn width_byte(width: f64) -> u8 {
    round(width * POINT_SCALE).clamp(1.0, f64::from(u8::MAX)) as u8
}

/// What a line over a column is and occupies there (pxl-portable ways.rs).
pub use pxl_portable::ways::{Crossing, Occupies, Occupy, STACKED_DECK_HEADROOM_M};

/// What a line is, from the source's flags alone; the byte is the wire's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Form {
    Bridge = ways::BRIDGE as u8,     // a slab under its surface
    Tunnel = ways::TUNNEL as u8,     // its clear height and shell over its surface
    Approach = ways::APPROACH as u8, // a bridge's or a tunnel's approach on the ground: an embankment or a cutting
}

impl Form {
    const ALL: [Form; 3] = [Form::Bridge, Form::Tunnel, Form::Approach];

    /// A byte `read_branch` let through.
    pub fn of(byte: u8) -> Form {
        Form::ALL[usize::from(byte)]
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Can stand below the ground.
    pub fn sinks(self) -> bool {
        self != Form::Bridge
    }

    /// Can stand above the ground.
    pub fn rises(self) -> bool {
        self != Form::Tunnel
    }
}

/// A resident record's room for lines and their points: the most a tile of
/// the format-10 set holds with the fold keeping every line its width keeps,
/// lines at z12 010331021012 and points at z15 010332102302331
/// (2026-10-15); a tile past either fails its bake (`Separation::fits`).
pub const MAX_LINES: usize = 1973;
pub const MAX_POINTS: usize = 4260;

/// Points a leaf of `Separation::boxes` covers, and children a node has: a ray
/// cast 20–25% faster than at 8 and twice 16's, for 16 KiB a record (measured
/// 2026-10-03, Namsan and Gangnam).
const FANOUT: usize = 4;

/// Boxes of the tree over `points` points: the leaves, then each level up to one.
pub const fn boxes_for(points: usize) -> usize {
    let (mut n, mut sum) = (points.div_ceil(FANOUT), 0);
    loop {
        sum += n;
        if n <= 1 {
            return sum;
        }
        n = n.div_ceil(FANOUT);
    }
}

pub const MAX_BOXES: usize = boxes_for(MAX_POINTS);

/// Whether a record's room holds `lines` lines of `points` points.
pub const fn holds(lines: usize, points: usize) -> bool {
    lines <= MAX_LINES && points <= MAX_POINTS
}

/// Holds no point.
const EMPTY: [i16; 4] = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];

/// A tile's lines, structure-of-arrays: a lane a line, then a lane a point.
#[derive(Clone, Debug, PartialEq)]
pub struct Separation {
    pub starts: Cow<'static, [u32]>,      // line d is points starts[d]..starts[d + 1]
    pub points: Cow<'static, [[i16; 2]]>, // centreline, 1/POINT_SCALE tile cell: the source's
    pub tops: Cow<'static, [i16]>,        // road surface, in leaf cells: solved over many tiles
    pub width: Cow<'static, [u8]>,        // 1/POINT_SCALE tile cell: the source's
    pub kind: Cow<'static, [u8]>,         // transport type: the source's class, the slab's material
    pub form: Cow<'static, [u8]>,         // `Form`: the source's flags
    pub clear: Cow<'static, [u8]>,        // a bore's clear space over its surface, in leaf cells: solved
    /// The coarsest zoom each point is held at (pxl-portable `ways::coarsest`):
    /// what a fold keeps of a line, the bake's alone — no client path folds,
    /// so a client's record holds none (`Lanes::into_room`).
    pub coarsest: Cow<'static, [u8]>,
    /// Derived: a tree of boxes over the points in order, each the segments it
    /// covers widened by their reach, in points; what `crossing_point` and
    /// `segments_over` walk.
    pub boxes: Cow<'static, [[i16; 4]]>,
}

impl Default for Separation {
    fn default() -> Self {
        Builder::default().finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub points: &'a [[i16; 2]],
    pub tops: &'a [i16],
    pub coarsest: &'a [u8],
    pub width: u8,
    pub kind: u8,
    pub form: Form,
    pub clear: u8,
}

impl Separation {
    /// Refused, with its lines and points, where a record's room does not
    /// hold it (`holds`).
    pub fn fits(&self) -> Result<(), [usize; 2]> {
        let size = [self.len(), self.points.len()];
        if holds(size[0], size[1]) {
            Ok(())
        } else {
            Err(size)
        }
    }

    pub fn len(&self) -> usize {
        self.width.len()
    }

    pub fn is_empty(&self) -> bool {
        self.width.is_empty()
    }

    #[inline]
    pub fn line(&self, d: usize) -> Line<'_> {
        let (a, b) = (self.starts[d] as usize, self.starts[d + 1] as usize);
        Line {
            points: &self.points[a..b],
            tops: &self.tops[a..b],
            coarsest: self.coarsest.get(a..b).unwrap_or_default(),
            width: self.width[d],
            kind: self.kind[d],
            form: Form::of(self.form[d]),
            clear: self.clear[d],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Line<'_>> {
        (0..self.len()).map(|d| self.line(d))
    }

    /// Each segment that may be over a column of a grid, by the point it
    /// starts at, with the column: column (i, j) the square of side `step`
    /// about `origin + (i + ½, j + ½) · step` in the tile's cells, a segment
    /// over it where the column's center is within its way's reach of its
    /// box. Row by row, each segment's in turn, in the points' order.
    pub fn segments_over(&self, origin: [f64; 2], step: f64, size: [i64; 2], mut each: impl FnMut(i64, i64, usize)) {
        let cell = |v: f64, k: usize| (v - origin[k]) / step - f64::from(CELL_CENTER);
        let far = [0, 1].map(|k| origin[k] + size[k] as f64 * step);
        let area = [origin[0], origin[1], far[0], far[1]].map(|v| v * POINT_SCALE);
        // Only the segments whose boxes reach the grid.
        for p in Near::new(&self.boxes, self.points.len(), area) {
            let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
            let line = self.line(d);
            let s = p - self.starts[d] as usize;
            if s + 1 >= line.points.len() {
                continue;
            }
            let half = line.reach();
            let (a, b) = (line.point(s), line.point(s + 1));
            let range = |k: usize| {
                let (lo, hi) = (cell(a[k].min(b[k]) - half, k).ceil() as i64, cell(a[k].max(b[k]) + half, k).floor() as i64);
                lo.max(0)..=hi.min(size[k] - 1)
            };
            for j in range(1) {
                for i in range(0) {
                    each(i, j, p);
                }
            }
        }
    }

    /// The segment that starts at point `p`, as the one rule reads it.
    pub fn segment(&self, p: usize) -> Segment {
        let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
        Segment::of(self.segment_words(p, p > self.starts[d] as usize))
    }

    /// The segment that starts at point `p` in its words (pxl-portable
    /// ways.rs `Segment::words`), and whether the one stored before it is its
    /// line's one before it: a line ends square at its first and last.
    pub fn segment_words(&self, p: usize, continues: bool) -> [u32; ways::SEGMENT_WORDS] {
        let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
        let (first, last) = (p == self.starts[d] as usize, p + 2 == self.starts[d + 1] as usize);
        let flags = ways::flags(u32::from(self.form[d]), first, last, continues);
        Segment::words(
            [self.points[p], self.points[p + 1]],
            [self.tops[p], self.tops[p + 1]],
            [self.width[d], self.kind[d], flags, self.clear[d]],
        )
    }

    /// Every line over each column of a grid (`segments_over`), each segment
    /// its own crossing, piers at the stations of the tile at `frame`.
    pub fn crossing_grid(&self, frame: Frame, origin: [f64; 2], step: f64, size: [i64; 2], mut each: impl FnMut(i64, i64, usize, Crossing)) {
        let stations = frame.stations();
        self.segments_over(origin, step, size, |i, j, p| {
            let at = [i, j].map(|v| v as f64 + f64::from(CELL_CENTER));
            let at = [origin[0] + at[0] * step, origin[1] + at[1] * step];
            if let Some(over) = crossing_at(&self.segment(p), at, step, &stations) {
                each(i, j, p, over);
            }
        });
    }

    /// Every line over the column of side `side` about `at`.
    pub fn crossing_point(&self, frame: Frame, at: [f64; 2], side: f64) -> impl Iterator<Item = Crossing> + '_ {
        let stations = frame.stations();
        Near::new(&self.boxes, self.points.len(), [at[0], at[1], at[0], at[1]].map(|v| v * POINT_SCALE)).filter_map(move |p| {
            let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
            (p + 1 < self.starts[d + 1] as usize).then(|| crossing_at(&self.segment(p), at, side, &stations)).flatten()
        })
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

    /// Each segment of a line that can rise or sink, by its line and its
    /// place in it, with every square of a tile of `side` cells cut `across`
    /// a side, row by row, that its footprint reaches: the one sweep a
    /// window's square's entries and its lines both read.
    pub fn reaches(&self, side: f64, across: u32, mut each: impl FnMut(usize, usize, &Line<'_>, usize)) {
        let step = side / f64::from(across);
        let square = |v: f64| ((v / step).floor() as i64).clamp(0, i64::from(across) - 1) as usize;
        for (d, line) in self.iter().enumerate().filter(|(_, d)| d.form.rises() || d.form.sinks()) {
            let half = line.footprint();
            for s in 0..line.points.len().saturating_sub(1) {
                let (a, b) = (line.point(s), line.point(s + 1));
                let ([x0, x1], [y0, y1]) = ([a[0].min(b[0]) - half, a[0].max(b[0]) + half], [a[1].min(b[1]) - half, a[1].max(b[1]) + half]);
                if x1 < 0.0 || y1 < 0.0 || x0 > side || y0 > side {
                    continue;
                }
                for y in square(y0)..=square(y1) {
                    for x in square(x0)..=square(x1) {
                        each(y * across as usize + x, d, &line, s);
                    }
                }
            }
        }
    }

    /// The lower and higher end of each segment whose footprint reaches the
    /// box, of the lines whose form `keeps`.
    fn segments_in(&self, keeps: fn(Form) -> bool, [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> impl Iterator<Item = [i16; 2]> + '_ {
        self.iter().filter(move |d| keeps(d.form)).flat_map(move |line| {
            let half = line.footprint();
            (0..line.points.len().saturating_sub(1)).filter_map(move |s| {
                let (a, b) = (line.point(s), line.point(s + 1));
                let apart = a[0].max(b[0]) + half < x0 || a[0].min(b[0]) - half > x1 || a[1].max(b[1]) + half < y0 || a[1].min(b[1]) - half > y1;
                let (ta, tb) = (line.tops[s], line.tops[s + 1]);
                (!apart).then_some([ta.min(tb), ta.max(tb)])
            })
        })
    }
}

impl Line<'_> {
    #[inline]
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.points[i].map(|v| f64::from(v) / POINT_SCALE)
    }

    pub fn width_cells(&self) -> f64 {
        f64::from(self.width) / POINT_SCALE
    }

    pub fn reach(&self) -> f64 {
        f64::from(way::reach(self.width_cells() as f32))
    }

    pub fn footprint(&self) -> f64 {
        footprint(self.width_cells())
    }

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
}

/// What segment `s` is over the column of side `side` about `at` in its
/// tile's cells (pxl-portable ways.rs `crossing`).
fn crossing_at(s: &Segment, at: [f64; 2], side: f64, stations: &Stations) -> Option<Crossing> {
    Some(ways::crossing(s, at.map(|v| v as f32), side as f32, stations)).filter(Crossing::is)
}

pub fn footprint(width: f64) -> f64 {
    f64::from(ways::footprint(width as f32))
}

/// The runs of consecutive segments of the line through `points`, in a
/// tile's cells, whose box comes within `reach` of the tile, as first and
/// last point: what a tile keeps of a line, a leaf's (pxl-geo-bake
/// terrain/pipeline/separation.rs) and a fold's alike, so tiles sharing a
/// line agree point for point.
pub fn runs_within(points: &[[f64; 2]], reach: f64) -> Vec<[usize; 2]> {
    let (lo, hi) = (-reach, f64::from(TILE_CELLS) + reach);
    let near = |a: [f64; 2], b: [f64; 2]| (0..2).all(|k| a[k].max(b[k]) >= lo && a[k].min(b[k]) <= hi);
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for s in 0..points.len().saturating_sub(1) {
        match (near(points[s], points[s + 1]), start) {
            (true, None) => start = Some(s),
            (false, Some(a)) => {
                runs.push([a, s]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(a) = start {
        runs.push([a, points.len() - 1]);
    }
    runs
}

/// Lines at `zoom`, one coarser, off the 4×4 tiles about its children
/// (pxl-spec space/layer/tile.rs `neighborhood`, a missing one none), each
/// segment once: every point is the child's halved and rounded (`round`,
/// half to even), a function of that point alone, so the copies tiles hold
/// of one line fold to the same segments and are taken as one. A line whose
/// width vanishes at the parent's cells (pxl-portable `ways::vanishes`) is
/// none there, and a segment whose ends fold to one point is none. The
/// segments rejoin where a copy held one after another and the second starts
/// where the first ends, surface and all, so a line runs on through its
/// copies' cuts; then of each line only the points held at `zoom`
/// (`ways::held_at`) stay, each by its own lane, so what a parent holds is a
/// function of the world's lines alone, not of which tiles held them. A line
/// holds a point where it crosses each edge of the tiles of the zoom under
/// one it is held at (geo-bake terrain/pipeline/separation.rs), so a
/// segment the parent holds lies in one child's tile, whose copy holds every
/// point under it: the 4×4 hold whole every segment the parent keeps. Each
/// line is kept where it runs within its footprint of the parent
/// (`runs_within`), so the ring a tile's lines guarantee holds at every
/// zoom. A record holding more than a tile's room is refused where it is
/// encoded, never cut here.
pub fn fold_separation(around: [Option<&Separation>; 16], zoom: Zoom) -> Separation {
    let tile = TILE_CELLS as i32 * POINT_SCALE as i32;
    let mut folded = Folded::default();
    for (k, child) in around.iter().enumerate() {
        let Some(child) = child else { continue };
        let q = [(k % 4) as i32 - 1, (k / 4) as i32 - 1].map(|q| q * tile);
        for line in child.iter() {
            let width = width_byte(line.width_cells() / 2.0);
            if ways::vanishes(f32::from(width) / ways::POINT_SCALE) {
                continue;
            }
            let up = |i: usize| ([0, 1].map(|a| round(f64::from(i32::from(line.points[i][a]) + q[a]) / 2.0) as i32), line.tops[i], line.coarsest[i]);
            let mut before = None;
            for i in 0..line.points.len() - 1 {
                let (a, b) = (up(i), up(i + 1));
                if a.0 == b.0 {
                    continue;
                }
                let s = folded.intern(Folding {
                    a,
                    b,
                    width,
                    kind: line.kind,
                    form: line.form.byte(),
                    clear: line.clear,
                });
                if let Some(p) = before.filter(|&p: &usize| folded.segments[p].b == a) {
                    folded.join(p, s);
                }
                before = Some(s);
            }
        }
    }
    folded.lines(zoom)
}

/// A folded segment: its ends in the parent's eighths of a cell with their
/// surface and the coarsest zoom each is held at, and its line's width,
/// kind, form and clear.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Folding {
    a: ([i32; 2], i16, u8),
    b: ([i32; 2], i16, u8),
    width: u8,
    kind: u8,
    form: u8,
    clear: u8,
}

/// No segment is known to come after (or before) one; more than one is.
const NO_SEGMENT: usize = usize::MAX;
const SEGMENTS: usize = usize::MAX - 1;

/// A fold's segments, each once, and the one after and before each.
#[derive(Default)]
struct Folded {
    segments: Vec<Folding>,
    index: fnv::FnvHashMap<Folding, usize>,
    next: Vec<usize>,
    prev: Vec<usize>,
}

impl Folded {
    fn intern(&mut self, s: Folding) -> usize {
        *self.index.entry(s).or_insert_with(|| {
            self.segments.push(s);
            self.next.push(NO_SEGMENT);
            self.prev.push(NO_SEGMENT);
            self.segments.len() - 1
        })
    }

    /// `s` after `p` in a copy, of one line where both have the same line.
    fn join(&mut self, p: usize, s: usize) {
        let same = |a: Folding, b: Folding| (a.width, a.kind, a.form, a.clear) == (b.width, b.kind, b.form, b.clear);
        if !same(self.segments[p], self.segments[s]) {
            return;
        }
        let one = |slot: &mut usize, v: usize| *slot = if *slot == NO_SEGMENT || *slot == v { v } else { SEGMENTS };
        one(&mut self.next[p], s);
        one(&mut self.prev[s], p);
    }

    /// Whether `s` is the only one after `p` and `p` the only one before it.
    fn joined(&self, p: usize, s: usize) -> bool {
        self.next[p] == s && self.prev[s] == p
    }

    /// The segments as lines, each from one no other is joined before, in
    /// the order they were met, a ring of them, joined all round, from the
    /// first of it met; of each, the points held at `zoom`.
    fn lines(&self, zoom: Zoom) -> Separation {
        let mut out = Builder::default();
        let mut taken = vec![false; self.segments.len()];
        let heads = (0..self.segments.len()).filter(|&s| self.prev[s] >= SEGMENTS || !self.joined(self.prev[s], s));
        for head in heads.chain(0..self.segments.len()) {
            if taken[head] {
                continue;
            }
            let mut chain = vec![self.segments[head].a];
            let mut s = head;
            loop {
                taken[s] = true;
                chain.push(self.segments[s].b);
                match self.next[s] {
                    t if t < SEGMENTS && self.joined(s, t) && !taken[t] => s = t,
                    _ => break,
                }
            }
            chain.retain(|p| ways::held_at(Zoom::from(p.2), zoom));
            chain.dedup_by(|p, q| p.0 == q.0);
            let f = self.segments[head];
            let at: Vec<[f64; 2]> = chain.iter().map(|p| p.0.map(|v| f64::from(v) / POINT_SCALE)).collect();
            for [a, b] in runs_within(&at, footprint(f64::from(f.width) / POINT_SCALE)) {
                out.push((a..=b).map(|i| (at[i], chain[i].1, chain[i].2)), f.width, f.kind, Form::of(f.form), f.clear);
            }
        }
        out.finish()
    }
}

#[derive(Default)]
pub struct Builder {
    starts: Vec<u32>,
    points: Vec<[i16; 2]>,
    tops: Vec<i16>,
    coarsest: Vec<u8>,
    width: Vec<u8>,
    kind: Vec<u8>,
    form: Vec<u8>,
    clear: Vec<u8>,
}

impl Builder {
    /// A line of fewer than two points, or of no width, is dropped; each
    /// point with its surface and the coarsest zoom it is held at.
    pub fn push(&mut self, points: impl IntoIterator<Item = ([f64; 2], i16, u8)>, width: u8, kind: u8, form: Form, clear: u8) {
        let at = self.points.len();
        for ([x, y], top, coarsest) in points {
            let q = |v: f64| round(v * POINT_SCALE).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            self.points.push([q(x), q(y)]);
            self.tops.push(top);
            self.coarsest.push(coarsest);
        }
        if self.points.len() - at < 2 || width == 0 {
            self.points.truncate(at);
            self.tops.truncate(at);
            self.coarsest.truncate(at);
            return;
        }
        self.starts.push(at as u32);
        self.width.push(width);
        self.kind.push(kind);
        self.form.push(form.byte());
        self.clear.push(clear);
    }

    pub fn finish(mut self) -> Separation {
        self.starts.push(self.points.len() as u32);
        let mut boxes = vec![EMPTY; boxes_for(self.points.len())];
        build_boxes(&self.starts, &self.points, &self.width, &mut boxes);
        Separation {
            starts: self.starts.into(),
            points: self.points.into(),
            tops: self.tops.into(),
            coarsest: self.coarsest.into(),
            width: self.width.into(),
            kind: self.kind.into(),
            form: self.form.into(),
            clear: self.clear.into(),
            boxes: boxes.into(),
        }
    }
}

// separation = [starts, points (flattened), tops, width, kind, form, clear, coarsest]
pub fn write_branch<'a>(separation: &'a Separation, b: &mut BranchViewImpl<'a>) {
    b.branch(Some(&mut |d| {
        d.u32_array(Some(&separation.starts))
            .i16_array(Some(separation.points.as_flattened()))
            .i16_array(Some(&separation.tops))
            .u8_array(Some(&separation.width))
            .u8_array(Some(&separation.kind))
            .u8_array(Some(&separation.form))
            .u8_array(Some(&separation.clear))
            .u8_array(Some(&separation.coarsest));
    }));
}

pub struct Lanes<'a> {
    starts: &'a [u32],
    points: &'a [[i16; 2]],
    tops: &'a [i16],
    width: &'a [u8],
    kind: &'a [u8],
    form: &'a [u8],
    clear: &'a [u8],
    coarsest: &'a [u8],
}

/// Refused unless the lanes are one tile's separation, consistently sized and within a record's room.
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
        width: d.at(3)?.get_array::<u8>()?,
        kind: d.at(4)?.get_array::<u8>()?,
        form: d.at(5)?.get_array::<u8>()?,
        clear: d.at(6)?.get_array::<u8>()?,
        coarsest: d.at(7)?.get_array::<u8>()?,
    };
    let n = lanes.width.len();
    let shaped = lanes.kind.len() == n
        && lanes.form.len() == n
        && lanes.clear.len() == n
        && starts.len() == n + 1
        && lanes.tops.len() == points.len()
        && lanes.coarsest.len() == points.len()
        && holds(n, points.len())
        && starts.first() == Some(&0)
        && starts.last() == Some(&(points.len() as u32))
        && starts.windows(2).all(|w| w[0] <= w[1])
        && lanes.form.iter().all(|&f| usize::from(f) < Form::ALL.len());
    shaped.then_some(lanes).ok_or_else(malformed)
}

impl Lanes<'_> {
    pub fn to_separation(&self) -> Separation {
        let mut boxes = vec![EMPTY; boxes_for(self.points.len())];
        build_boxes(self.starts, self.points, self.width, &mut boxes);
        Separation {
            starts: self.starts.to_vec().into(),
            points: self.points.to_vec().into(),
            tops: self.tops.to_vec().into(),
            coarsest: self.coarsest.to_vec().into(),
            width: self.width.to_vec().into(),
            kind: self.kind.to_vec().into(),
            form: self.form.to_vec().into(),
            clear: self.clear.to_vec().into(),
            boxes: boxes.into(),
        }
    }

    /// Into a client's record: every lane but `coarsest`, which only a fold
    /// reads, and only the bake folds.
    pub fn into_room(&self, room: Room) -> Separation {
        fn put<T: Copy>(dst: &'static mut [T], src: &[T]) -> &'static [T] {
            let dst = &mut dst[..src.len()];
            dst.copy_from_slice(src);
            dst
        }
        let boxes = &mut room.boxes[..boxes_for(self.points.len())];
        build_boxes(self.starts, self.points, self.width, boxes);
        Separation {
            starts: put(room.starts, self.starts).into(),
            points: put(room.points, self.points).into(),
            tops: put(room.tops, self.tops).into(),
            coarsest: Cow::Borrowed(&[]),
            width: put(room.width, self.width).into(),
            kind: put(room.kind, self.kind).into(),
            form: put(room.form, self.form).into(),
            clear: put(room.clear, self.clear).into(),
            boxes: (&*boxes).into(),
        }
    }
}

/// Fills `out`, `boxes_for(points.len())` long: a leaf the segments starting
/// at its points, each widened by its line's reach; a node its children's union.
fn build_boxes(starts: &[u32], points: &[[i16; 2]], width: &[u8], out: &mut [[i16; 4]]) {
    let n = points.len();
    let mut d = 0;
    for (k, b) in out[..n.div_ceil(FANOUT)].iter_mut().enumerate() {
        *b = EMPTY;
        for i in k * FANOUT..((k + 1) * FANOUT).min(n) {
            while starts[d + 1] as usize <= i {
                d += 1;
            }
            if i + 1 >= starts[d + 1] as usize {
                continue;
            }
            // Within reach of the segment is within its box widened by the reach.
            let r = (f64::from(way::reach(f32::from(width[d]) / ways::POINT_SCALE)) * POINT_SCALE).ceil() as i32;
            let (a, c) = (points[i], points[i + 1]);
            let lo = |k: usize| (i32::from(a[k].min(c[k])) - r).max(i32::from(i16::MIN)) as i16;
            let hi = |k: usize| (i32::from(a[k].max(c[k])) + r).min(i32::from(i16::MAX)) as i16;
            *b = union(*b, [lo(0), lo(1), hi(0), hi(1)]);
        }
    }
    let (mut below, mut len) = (0, n.div_ceil(FANOUT));
    while len > 1 {
        let up = len.div_ceil(FANOUT);
        for k in 0..up {
            let children = below + k * FANOUT..below + ((k + 1) * FANOUT).min(len);
            out[below + len + k] = out[children].iter().fold(EMPTY, |u, &b| union(u, b));
        }
        (below, len) = (below + len, up);
    }
}

fn union(a: [i16; 4], b: [i16; 4]) -> [i16; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// The points whose segment's box meets `area`, low corner then high, in
/// order: the tree walked depth first.
struct Near<'a> {
    boxes: &'a [[i16; 4]],
    area: [f64; 4],
    /// each level's first box, leaves at 0
    level_at: [usize; 8],
    stack: [(u8, u16); 8 * FANOUT],
    len: usize,
    leaf: core::ops::Range<usize>,
    points: usize,
}

impl<'a> Near<'a> {
    fn new(boxes: &'a [[i16; 4]], points: usize, area: [f64; 4]) -> Near<'a> {
        let mut near = Near {
            boxes,
            area,
            level_at: [0; 8],
            stack: [(0, 0); 8 * FANOUT],
            len: 0,
            leaf: 0..0,
            points,
        };
        let (mut level, mut len) = (0, points.div_ceil(FANOUT));
        while len > 1 {
            near.level_at[level + 1] = near.level_at[level] + len;
            (level, len) = (level + 1, len.div_ceil(FANOUT));
        }
        if len == 1 {
            near.stack[0] = (level as u8, 0);
            near.len = 1;
        }
        near
    }

    fn meets(&self, b: [i16; 4]) -> bool {
        let a = self.area;
        f64::from(b[0]) <= a[2] && a[0] <= f64::from(b[2]) && f64::from(b[1]) <= a[3] && a[1] <= f64::from(b[3])
    }
}

impl Iterator for Near<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            if let Some(i) = self.leaf.next() {
                return Some(i);
            }
            if self.len == 0 {
                return None;
            }
            self.len -= 1;
            let (level, k) = self.stack[self.len];
            let (level, k) = (level as usize, k as usize);
            if !self.meets(self.boxes[self.level_at[level] + k]) {
                continue;
            }
            if level == 0 {
                self.leaf = k * FANOUT..((k + 1) * FANOUT).min(self.points);
                continue;
            }
            // children pushed last first, so they come off in order
            let below = self.points.div_ceil(FANOUT.pow(level as u32));
            for c in (k * FANOUT..((k + 1) * FANOUT).min(below)).rev() {
                self.stack[self.len] = ((level - 1) as u8, c as u16);
                self.len += 1;
            }
        }
    }
}

/// A record's room for lines, each lane `MAX_LINES` or `MAX_POINTS` long, and `MAX_BOXES` for the boxes.
pub struct Room {
    pub starts: &'static mut [u32],
    pub points: &'static mut [[i16; 2]],
    pub tops: &'static mut [i16],
    pub width: &'static mut [u8],
    pub kind: &'static mut [u8],
    pub form: &'static mut [u8],
    pub clear: &'static mut [u8],
    pub boxes: &'static mut [[i16; 4]],
}

#[cfg(test)]
#[path = "separation.test.rs"]
mod test;
