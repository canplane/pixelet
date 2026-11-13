// pxl-spec/bake/separation.rs

//! A tile's ways off the ground: polylines in the tile's cells with their road
//! surface in leaf cells at each point, solved offline over the whole network.
//! What they look like — slab, piers, bore, embankment, cutting — the client
//! makes from the surface and these numbers.

use std::collections::TryReserveError;

use xpute_kit::status::error::MarshalError;
use xpute_kit::wire::xtp::shape::Shape;

use super::malformed;
use crate::procedural::Frame;
use crate::space::coord::zoom::Zoom;
use crate::space::layer::lattice::{CELL_CENTER, TILE_CELLS};
use crate::way::{self, Crossing, Segment, StationGrid};
use xpute_kit::math::scalar::round;

/// Points are in eighths of the tile's cell, which an `i16` holds far past
/// the tile's edge (way.rs).
pub const POINT_SCALE: f64 = way::POINT_SCALE as f64;

/// A line's width byte for `width` of its tile's cells, in eighths rounded
/// half to even, an eighth at least: the widest a way is drawn is what the
/// byte holds. The ground draws a way no wider, so it is one width on and off
/// a bridge, and how far past a tile anything drawn on it can come from is
/// bounded.
pub fn width_byte(width: f64) -> u8 {
    round(width * POINT_SCALE).clamp(1.0, f64::from(u8::MAX)) as u8
}

/// What a line is, from the source's flags alone; the byte is the wire's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Form {
    Bridge = way::BRIDGE as u8,     // A slab, which stands under its surface.
    Tunnel = way::TUNNEL as u8,     // Its clear height and its shell stand over its surface.
    Approach = way::APPROACH as u8, // A bridge's or a tunnel's approach on the ground: an embankment or a cutting.
}

impl Form {
    const ALL: [Form; 3] = [Form::Bridge, Form::Tunnel, Form::Approach];

    /// A byte `Table::of` let through.
    pub fn of(byte: u8) -> Form {
        Form::ALL[usize::from(byte)]
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Whether a line of this form can stand below the ground.
    pub fn sinks(self) -> bool {
        self != Form::Bridge
    }

    /// Whether a line of this form can stand above the ground.
    pub fn rises(self) -> bool {
        self != Form::Tunnel
    }
}

/// A tile's lines, structure-of-arrays: a column a line, then a column a point.
#[derive(Clone, Debug, PartialEq)]
pub struct Separation {
    pub starts: Vec<u32>,      // Line d holds the points starts[d]..starts[d + 1].
    pub points: Vec<[i16; 2]>, // Each point of the centerline, in 1/POINT_SCALE of a tile cell, as the source gives it.
    pub heights: Vec<i16>, // The road surface at each point, in leaf cells, solved over many tiles.
    pub widths: Vec<u8>, // Each line's width, in 1/POINT_SCALE of a tile cell, as the source gives it.
    pub asset_ids: Vec<u8>, // Each line's transport type: the source's class, which is the slab's material.
    pub forms: Vec<u8>,     // Each line's `Form`, from the source's flags.
    pub clearances: Vec<u8>, // A bore's clear space over its surface, in leaf cells, as the solve set it.
    /// The coarsest zoom at which each point is held (`way::coarsest`): what a fold
    /// keeps of a line. Only the bake reads it, since no client path folds, so a client holds
    /// none (`Table::to_client`).
    pub first_zooms: Vec<u8>,
}

impl Default for Separation {
    fn default() -> Self {
        Builder::default().finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    pub points: &'a [[i16; 2]],
    pub heights: &'a [i16],
    pub first_zooms: &'a [u8],
    pub width: u8,
    pub asset_id: u8,
    pub form: Form,
    pub clearance: u8,
}

impl Separation {
    pub fn len(&self) -> usize {
        self.widths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    #[inline]
    pub fn line(&self, d: usize) -> Line<'_> {
        let (a, b) = (self.starts[d] as usize, self.starts[d + 1] as usize);
        Line {
            points: &self.points[a..b],
            heights: &self.heights[a..b],
            first_zooms: self.first_zooms.get(a..b).unwrap_or_default(),
            width: self.widths[d],
            asset_id: self.asset_ids[d],
            form: Form::of(self.forms[d]),
            clearance: self.clearances[d],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Line<'_>> {
        (0..self.len()).map(|d| self.line(d))
    }

    /// Each segment that may be over a column of a grid, by the point it starts at, with the
    /// column. Column (i, j) is the brick of side `step` around `origin + (i + ½, j + ½) · step`
    /// in the tile's cells, and a segment is over it where the column's center lies within its
    /// way's half width of the segment's box. The columns come row by row, each column's segments
    /// in the points' order.
    pub(crate) fn segments_over(
        &self,
        origin: [f64; 2],
        step: f64,
        size: [i64; 2],
        mut each: impl FnMut(i64, i64, usize),
    ) {
        let cell = |v: f64, k: usize| (v - origin[k]) / step - f64::from(CELL_CENTER);
        for d in 0..self.starts.len() - 1 {
            let line = self.line(d);
            let half = line.half_width();
            for s in 0..line.points.len().saturating_sub(1) {
                let (a, b) = (line.point(s), line.point(s + 1));
                let range = |k: usize| {
                    let (lo, hi) = (
                        cell(a[k].min(b[k]) - half, k).ceil() as i64,
                        cell(a[k].max(b[k]) + half, k).floor() as i64,
                    );
                    lo.max(0)..=hi.min(size[k] - 1)
                };
                for j in range(1) {
                    for i in range(0) {
                        each(i, j, self.starts[d] as usize + s);
                    }
                }
            }
        }
    }

    /// The segment that starts at point `p`, as the one rule reads it.
    pub fn segment(&self, p: usize) -> Segment {
        let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
        Segment::of(self.segment_words(p, p > self.starts[d] as usize))
    }

    /// The segment that starts at point `p`, in its words (way.rs `Segment::words`),
    /// with whether the one stored before it is its line's one before it; a line ends square at
    /// its first and last.
    pub(crate) fn segment_words(&self, p: usize, continues: bool) -> [u32; way::SEGMENT_WORDS] {
        let d = self.starts.partition_point(|&s| s as usize <= p) - 1;
        let (first, last) = (
            p == self.starts[d] as usize,
            p + 2 == self.starts[d + 1] as usize,
        );
        let flags = way::flags(u32::from(self.forms[d]), first, last, continues);
        Segment::words(
            [self.points[p], self.points[p + 1]],
            [self.heights[p], self.heights[p + 1]],
            [self.widths[d], self.asset_ids[d], flags, self.clearances[d]],
        )
    }

    /// Every line over each column of a grid of the tile's cells from `origin`
    /// (`segments_over`), each segment its own crossing, with piers at the stations of the tile at
    /// `frame`.
    pub(crate) fn crossing_grid(
        &self,
        frame: Frame,
        origin: [i32; 2],
        size: [i64; 2],
        mut each: impl FnMut(i64, i64, usize, Crossing),
    ) {
        let stations = frame.stations();
        let eighths = POINT_SCALE as i32;
        let center = (CELL_CENTER * way::POINT_SCALE) as i32;
        self.segments_over(origin.map(f64::from), 1.0, size, |i, j, p| {
            let at = [i, j].map(|v| v as i32);
            let at = [0, 1].map(|k| (origin[k] + at[k]) * eighths + center);
            if let Some(over) = crossing_at(&self.segment(p), at, &stations) {
                each(i, j, p, over);
            }
        });
    }

    /// Each segment of a line that can rise or sink, by its line and its place in it, with every
    /// brick that its footprint reaches, of a tile of `side` cells cut `across` bricks a side, row
    /// by row. A clip level's brick's entries and its lines both read this one sweep.
    pub fn reaches(
        &self,
        side: f64,
        across: u32,
        mut each: impl FnMut(usize, usize, &Line<'_>, usize),
    ) {
        let step = side / f64::from(across);
        let brick = |v: f64| ((v / step).floor() as i64).clamp(0, i64::from(across) - 1) as usize;
        for (d, line) in self
            .iter()
            .enumerate()
            .filter(|(_, d)| d.form.rises() || d.form.sinks())
        {
            let half = line.footprint();
            for s in 0..line.points.len().saturating_sub(1) {
                let (a, b) = (line.point(s), line.point(s + 1));
                let ([x0, x1], [y0, y1]) = (
                    [a[0].min(b[0]) - half, a[0].max(b[0]) + half],
                    [a[1].min(b[1]) - half, a[1].max(b[1]) + half],
                );
                if x1 < 0.0 || y1 < 0.0 || x0 > side || y0 > side {
                    continue;
                }
                for y in brick(y0)..=brick(y1) {
                    for x in brick(x0)..=brick(x1) {
                        each(y * across as usize + x, d, &line, s);
                    }
                }
            }
        }
    }
}

impl Line<'_> {
    #[inline]
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.points[i].map(|v| f64::from(v) / POINT_SCALE)
    }

    pub(crate) fn width_cells(&self) -> f64 {
        f64::from(self.width) / POINT_SCALE
    }

    pub fn half_width(&self) -> f64 {
        f64::from(way::half_width(self.width_cells() as f32))
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

/// What segment `s` is over the column about `at`, in eighths of its tile's
/// cells (way.rs `crossing`).
fn crossing_at(s: &Segment, at: [i32; 2], stations: &StationGrid) -> Option<Crossing> {
    Some(way::crossing(s, at, stations)).filter(Crossing::is)
}

pub fn footprint(width: f64) -> f64 {
    f64::from(way::footprint(width as f32))
}

/// The runs of consecutive segments of the line through `points`, in a
/// tile's cells, whose box comes within `margin` of the tile, as first and
/// last point: what a tile keeps of a line, a leaf's (pxl-geo-bake
/// terrain/pipeline/separation.rs) and a fold's alike, so tiles sharing a
/// line agree point for point.
pub fn runs_within(points: &[[f64; 2]], margin: f64) -> Vec<[usize; 2]> {
    let (lo, hi) = (-margin, f64::from(TILE_CELLS) + margin);
    let near =
        |a: [f64; 2], b: [f64; 2]| (0..2).all(|k| a[k].max(b[k]) >= lo && a[k].min(b[k]) <= hi);
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

/// Lines at `zoom`, one coarser, from the 4×4 tiles around its children (pxl-spec
/// space/layer/quadrant.rs `neighborhood`; a missing one counts as none), each segment once.
/// Every point is the child's halved and rounded (`round`, half to even), a function of that
/// point alone, so the copies of one line that tiles hold fold to the same segments and are
/// taken as one. A line whose width vanishes at the parent's cells
/// (`way::vanishes`) is none there, and so is a segment whose ends fold to one point. The
/// segments rejoin where a copy held one after another and the second starts where the first
/// ends, surface and all, so a line runs on through the cuts between its copies. Then, of each
/// line, only the points held at `zoom` (from its `way::coarsest` on) stay, each by its own
/// column, so what a parent holds depends on the world's lines alone, not on which tiles held
/// them. A line holds a point where it crosses each edge of the tiles of the zoom under one it
/// is held at (pxl-geo-bake terrain/pipeline/separation.rs), so a segment the parent holds
/// lies in one child's tile, whose copy holds every point under it; the 4×4 tiles hold every
/// segment the parent keeps whole. Each line is kept where it runs within its footprint of the
/// parent (`runs_within`), so the ring a tile's lines guarantee holds at every zoom. A record
/// holding more than a tile's packet is refused where it is encoded, never cut here.
pub fn fold_separation(around: [Option<&Separation>; 16], zoom: Zoom) -> Separation {
    let tile = TILE_CELLS as i32 * POINT_SCALE as i32;
    let mut folded = Fold::default();
    for (k, child) in around.iter().enumerate() {
        let Some(child) = child else { continue };
        let q = [(k % 4) as i32 - 1, (k / 4) as i32 - 1].map(|q| q * tile);
        for line in child.iter() {
            let width = width_byte(line.width_cells() / 2.0);
            if way::vanishes(f32::from(width) / way::POINT_SCALE) {
                continue;
            }
            let up = |i: usize| {
                (
                    [0, 1].map(|a| {
                        round(f64::from(i32::from(line.points[i][a]) + q[a]) / 2.0) as i32
                    }),
                    line.heights[i],
                    line.first_zooms[i],
                )
            };
            let mut before = None;
            for i in 0..line.points.len() - 1 {
                let (a, b) = (up(i), up(i + 1));
                if a.0 == b.0 {
                    continue;
                }
                let s = folded.intern(FoldSegment {
                    a,
                    b,
                    width,
                    asset_id: line.asset_id,
                    form: line.form.byte(),
                    clearance: line.clearance,
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
/// asset_id, form and clearance.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FoldSegment {
    a: ([i32; 2], i16, u8),
    b: ([i32; 2], i16, u8),
    width: u8,
    asset_id: u8,
    form: u8,
    clearance: u8,
}

/// Marks that no single segment is known to come after (or before) one: none, or more than
/// one.
const NO_SEGMENT: usize = usize::MAX;
const SEGMENTS: usize = usize::MAX - 1;

/// A fold's segments, each once, and the one after and before each.
#[derive(Default)]
struct Fold {
    segments: Vec<FoldSegment>,
    idx: fnv::FnvHashMap<FoldSegment, usize>,
    next: Vec<usize>,
    prev: Vec<usize>,
}

impl Fold {
    fn intern(&mut self, s: FoldSegment) -> usize {
        *self.idx.entry(s).or_insert_with(|| {
            self.segments.push(s);
            self.next.push(NO_SEGMENT);
            self.prev.push(NO_SEGMENT);
            self.segments.len() - 1
        })
    }

    /// Records that `s` comes after `p` in a copy, where both are of the same line.
    fn join(&mut self, p: usize, s: usize) {
        let same = |a: FoldSegment, b: FoldSegment| {
            (a.width, a.asset_id, a.form, a.clearance) == (b.width, b.asset_id, b.form, b.clearance)
        };
        if !same(self.segments[p], self.segments[s]) {
            return;
        }
        let one = |slot: &mut usize, v: usize| {
            *slot = if *slot == NO_SEGMENT || *slot == v {
                v
            } else {
                SEGMENTS
            }
        };
        one(&mut self.next[p], s);
        one(&mut self.prev[s], p);
    }

    /// Whether `s` is the only one after `p` and `p` the only one before it.
    fn is_joined(&self, p: usize, s: usize) -> bool {
        self.next[p] == s && self.prev[s] == p
    }

    /// The segments as lines, each starting from a segment no other is joined before, in the
    /// order they were met; a ring of them, joined all around, starts from its first segment met.
    /// Of each line, only the points held at `zoom` are kept.
    fn lines(&self, zoom: Zoom) -> Separation {
        let mut out = Builder::default();
        let mut taken = vec![false; self.segments.len()];
        let heads = (0..self.segments.len())
            .filter(|&s| self.prev[s] >= SEGMENTS || !self.is_joined(self.prev[s], s));
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
                    t if t < SEGMENTS && self.is_joined(s, t) && !taken[t] => s = t,
                    _ => break,
                }
            }
            chain.retain(|p| Zoom::from(p.2) <= zoom);
            chain.dedup_by(|p, q| p.0 == q.0);
            let f = self.segments[head];
            let at: Vec<[f64; 2]> = chain
                .iter()
                .map(|p| p.0.map(|v| f64::from(v) / POINT_SCALE))
                .collect();
            for [a, b] in runs_within(&at, footprint(f64::from(f.width) / POINT_SCALE)) {
                out.push(
                    (a..=b).map(|i| (at[i], chain[i].1, chain[i].2)),
                    f.width,
                    f.asset_id,
                    Form::of(f.form),
                    f.clearance,
                );
            }
        }
        out.finish()
    }
}

#[derive(Default)]
pub struct Builder {
    starts: Vec<u32>,
    points: Vec<[i16; 2]>,
    heights: Vec<i16>,
    first_zooms: Vec<u8>,
    widths: Vec<u8>,
    asset_ids: Vec<u8>,
    forms: Vec<u8>,
    clearances: Vec<u8>,
}

impl Builder {
    /// A line of fewer than two points, or of no width, is dropped; each
    /// point with its surface and the coarsest zoom it is held at.
    pub fn push(
        &mut self,
        points: impl IntoIterator<Item = ([f64; 2], i16, u8)>,
        width: u8,
        asset_id: u8,
        form: Form,
        clearance: u8,
    ) {
        let at = self.points.len();
        for ([x, y], height, first_zoom) in points {
            let q = |v: f64| {
                round(v * POINT_SCALE).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
            };
            self.points.push([q(x), q(y)]);
            self.heights.push(height);
            self.first_zooms.push(first_zoom);
        }
        if self.points.len() - at < 2 || width == 0 {
            self.points.truncate(at);
            self.heights.truncate(at);
            self.first_zooms.truncate(at);
            return;
        }
        self.starts.push(at as u32);
        self.widths.push(width);
        self.asset_ids.push(asset_id);
        self.forms.push(form.byte());
        self.clearances.push(clearance);
    }

    pub fn finish(mut self) -> Separation {
        self.starts.push(self.points.len() as u32);
        Separation {
            starts: self.starts,
            points: self.points,
            heights: self.heights,
            first_zooms: self.first_zooms,
            widths: self.widths,
            asset_ids: self.asset_ids,
            forms: self.forms,
            clearances: self.clearances,
        }
    }
}

/// The packet's lanes: starts, points (flattened), heights, widths, asset_ids, forms, clearances, first_zooms.
pub(crate) type Packet = (
    Vec<u32>,
    Vec<i16>,
    Vec<i16>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
);

impl Separation {
    pub(crate) fn pkt(&self) -> <Packet as Shape>::Write<'_> {
        (
            &self.starts,
            self.points.as_flattened(),
            &self.heights,
            &self.widths,
            &self.asset_ids,
            &self.forms,
            &self.clearances,
            &self.first_zooms,
        )
    }
}

pub struct Table<'a> {
    starts: &'a [u32],
    points: &'a [[i16; 2]],
    heights: &'a [i16],
    widths: &'a [u8],
    asset_ids: &'a [u8],
    forms: &'a [u8],
    clearances: &'a [u8],
    first_zooms: &'a [u8],
}

impl<'a> Table<'a> {
    /// Refused unless the columns are one tile's separation, consistently sized.
    pub fn of(
        (starts, points, heights, widths, asset_ids, forms, clearances, first_zooms): <Packet as Shape>::Read<
            'a,
        >,
    ) -> Result<Table<'a>, MarshalError> {
        let (points, []) = points.as_chunks::<2>() else {
            return Err(malformed());
        };
        let n = widths.len();
        let shaped = asset_ids.len() == n
            && forms.len() == n
            && clearances.len() == n
            && starts.len() == n + 1
            && heights.len() == points.len()
            && first_zooms.len() == points.len()
            && starts.first() == Some(&0)
            && starts.last() == Some(&(points.len() as u32))
            && starts.windows(2).all(|w| w[0] <= w[1])
            && forms.iter().all(|&f| usize::from(f) < Form::ALL.len());
        shaped
            .then_some(Table {
                starts,
                points,
                heights,
                widths,
                asset_ids,
                forms,
                clearances,
                first_zooms,
            })
            .ok_or_else(malformed)
    }

    pub(crate) fn to_separation(&self) -> Separation {
        Separation {
            starts: self.starts.to_vec(),
            points: self.points.to_vec(),
            heights: self.heights.to_vec(),
            first_zooms: self.first_zooms.to_vec(),
            widths: self.widths.to_vec(),
            asset_ids: self.asset_ids.to_vec(),
            forms: self.forms.to_vec(),
            clearances: self.clearances.to_vec(),
        }
    }

    /// A client's own columns, on the heap and refused without a panic: every column but
    /// `first_zooms`, which only a fold reads; a client reads a brick's lines whole
    /// (bake/tile/terrain.rs `brick_lines`).
    pub(crate) fn to_client(&self) -> Result<Separation, TryReserveError> {
        fn column<T: Copy>(src: &[T]) -> Result<Vec<T>, TryReserveError> {
            let mut column = Vec::new();
            column.try_reserve_exact(src.len())?;
            column.extend_from_slice(src);
            Ok(column)
        }
        Ok(Separation {
            starts: column(self.starts)?,
            points: column(self.points)?,
            heights: column(self.heights)?,
            first_zooms: Vec::new(),
            widths: column(self.widths)?,
            asset_ids: column(self.asset_ids)?,
            forms: column(self.forms)?,
            clearances: column(self.clearances)?,
        })
    }
}

#[cfg(test)]
#[path = "separation.test.rs"]
mod test;
