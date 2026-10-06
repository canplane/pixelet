// pxl-spec/bake/deck.rs

//! A tile's ways off the ground: polylines in the tile's cells with their road
//! surface in leaf cells at each point, solved offline over the whole network.
//! What they look like — slab, piers, bore, embankment, cutting — the client
//! makes from the surface and these numbers.

use std::borrow::Cow;

use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, BranchViewImpl, NodeView};

use super::{malformed, way};
use crate::procedural::{pier_covers, Frame};
use crate::space::hierarchy::child_of;
use crate::space::pyramid::{GROUND, TILE_CELLS};
use crate::space::zoom::Zoom;
use xpute_core::math::scalar::round;

/// Points are in eighths of the tile's cell, which an `i16` holds far past the tile's edge.
pub const POINT_SCALE: f64 = 8.0;

/// The farthest past half its width the client draws a line, in its tile's
/// cells; the bake sizes a line's tile and extent by it.
pub const LINE_PAST: f64 = 1.0;

/// The widest a way is drawn, in its tile's cells: what a deck's width byte
/// holds. The ground draws a way no wider, so it is one width on and off a
/// bridge, and how far past a tile anything drawn on it can come from is bounded.
pub const WIDEST: f64 = u8::MAX as f64 / POINT_SCALE;

/// A deck's slab under its surface, in ground metres: Arnis's `DECK_STRUCTURE_DEPTH`.
pub const DECK_THICKNESS_M: f64 = 2.0;

/// Clear height a deck keeps over a deck under it, in ground metres: Arnis's
/// `STACKED_DECK_HEADROOM`.
pub const STACKED_DECK_HEADROOM_M: f64 = 6.0;

/// A deck's slab in cells, at least one.
pub fn slab_depth(cell_per_m: f64) -> i32 {
    (round(DECK_THICKNESS_M * cell_per_m) as i32).max(1)
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

/// Points a leaf of `Decks::boxes` covers, and children a node has: a ray
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

/// Holds no point.
const EMPTY: [i16; 4] = [i16::MAX, i16::MAX, i16::MIN, i16::MIN];

/// A tile's lines, structure-of-arrays: a lane a line, then a lane a point.
#[derive(Clone, Debug, PartialEq)]
pub struct Decks {
    pub starts: Cow<'static, [u32]>,      // line d is points starts[d]..starts[d + 1]
    pub points: Cow<'static, [[i16; 2]]>, // centreline, 1/POINT_SCALE tile cell: the source's
    pub tops: Cow<'static, [i16]>,        // road surface, in leaf cells: solved over many tiles
    pub width: Cow<'static, [u8]>,        // 1/POINT_SCALE tile cell: the source's
    pub kind: Cow<'static, [u8]>,         // transport type: the source's class, the slab's material
    pub form: Cow<'static, [u8]>,         // `Form`: the source's flags
    pub clear: Cow<'static, [u8]>,        // a bore's clear space over its surface, in leaf cells: solved
    /// Derived: a tree of boxes over the points in order, each the segments it
    /// covers widened by their reach, in points; what `over_point` walks.
    pub boxes: Cow<'static, [[i16; 4]]>,
}

impl Default for Decks {
    fn default() -> Self {
        Builder::default().finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Deck<'a> {
    pub points: &'a [[i16; 2]],
    pub tops: &'a [i16],
    pub width: u8,
    pub kind: u8,
    pub form: Form,
    pub clear: u8,
}

/// A line over one column: the lowest and highest its road surface runs
/// across the column, in leaf cells, and whether its pier's section covers it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub low: f64,
    pub top: f64,
    pub kind: u8,
    pub form: Form,
    pub pier: bool,
    pub clear: u8,
}

/// What a line occupies of one column, in leaf cells, each `[lo, hi)`; the
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
    /// `cell_per_m`: leaf cells a ground metre on the column's tile.
    pub fn occupies(&self, ground: i32, cell_per_m: f64) -> Occupies {
        // all the surface sweeps across the column, so a slab or a bore steps down to the
        // next column's without a gap however steep it runs
        let (low, top) = (round(self.low) as i32, round(self.top) as i32);
        match self.form {
            Form::Deck => {
                let lo = low - slab_depth(cell_per_m);
                Occupies::Above {
                    slab: [lo, top],
                    pier: (self.pier && lo > ground).then_some([ground, lo]),
                }
            }
            Form::Bore => Occupies::Through {
                air: [low, top + i32::from(self.clear)],
            },
            // a way on the ground moves the ground with it: filled up to it, cut down to it
            Form::Grade if top > ground => Occupies::Above { slab: [ground, top], pier: None },
            Form::Grade if top < ground => Occupies::Through { air: [low, ground] },
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
            width: self.width[d],
            kind: self.kind[d],
            form: Form::of(self.form[d]),
            clear: self.clear[d],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Deck<'_>> {
        (0..self.len()).map(|d| self.deck(d))
    }

    /// Every deck over each column of a grid: column (i, j) the square of side
    /// `step` about `origin + (i + ½, j + ½) · step` in the tile's cells. A
    /// column at a joint is reported for both segments; the caller joins them.
    pub fn over_grid(&self, frame: Option<Frame>, origin: [f64; 2], step: f64, size: [i64; 2], mut each: impl FnMut(i64, i64, usize, Over)) {
        let cell = |v: f64, k: usize| (v - origin[k]) / step - 0.5;
        for (d, deck) in self.iter().enumerate() {
            let half = deck.reach();
            for s in 0..deck.points.len().saturating_sub(1) {
                let (a, b) = (deck.point(s), deck.point(s + 1));
                let range = |k: usize| {
                    let (lo, hi) = (cell(a[k].min(b[k]) - half, k).ceil() as i64, cell(a[k].max(b[k]) + half, k).floor() as i64);
                    lo.max(0)..=hi.min(size[k] - 1)
                };
                for j in range(1) {
                    for i in range(0) {
                        let at = [origin[0] + (i as f64 + 0.5) * step, origin[1] + (j as f64 + 0.5) * step];
                        if let Some(over) = deck.over_segment(frame, s, at, step) {
                            each(i, j, d, over);
                        }
                    }
                }
            }
        }
    }

    /// Every deck over the column of side `side` about `at`.
    pub fn over_point(&self, frame: Option<Frame>, at: [f64; 2], side: f64) -> impl Iterator<Item = Over> + '_ {
        Near::new(&self.boxes, self.points.len(), at.map(|v| v * POINT_SCALE)).filter_map(move |i| {
            let d = self.starts.partition_point(|&s| s as usize <= i) - 1;
            let deck = self.deck(d);
            let s = i - self.starts[d] as usize;
            (s + 1 < deck.points.len()).then(|| deck.over_segment(frame, s, at, side)).flatten()
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
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.points[i].map(|v| f64::from(v) / POINT_SCALE)
    }

    pub fn width_cells(&self) -> f64 {
        f64::from(self.width) / POINT_SCALE
    }

    pub fn reach(&self) -> f64 {
        way::reach(self.width_cells())
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

    /// What segment `s` is over the column of side `side` about `at`, if the
    /// deck covers `at`: the surface it runs through across the column, and,
    /// given where the tile lies, whether `at` is inside the section of a pier
    /// at a station of this segment.
    fn over_segment(&self, frame: Option<Frame>, s: usize, at: [f64; 2], side: f64) -> Option<Over> {
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
        if !way::covers(self.width_cells(), libm::hypot(at[0] - near[0], at[1] - near[1])) {
            return None;
        }
        // the column's square spans this much of the segment either side of `t`
        let reach = if len2 > 0.0 { side / 2.0 * (dx.abs() + dy.abs()) / len2 } else { 0.0 };
        let z = |t: f64| f64::from(self.tops[s]) + (f64::from(self.tops[s + 1]) - f64::from(self.tops[s])) * t.clamp(0.0, 1.0);
        let (z0, z1) = (z(t - reach), z(t + reach));
        Some(Over {
            low: z0.min(z1),
            top: z0.max(z1),
            kind: self.kind,
            form: self.form,
            pier: self.form != Form::Bore && frame.is_some_and(|f| pier_covers(f, self.width_cells(), a, [dx, dy], near, at)),
            clear: self.clear,
        })
    }
}

/// Where the nearest point of the line from `a` along `d` to `p` lies, as a
/// share of `d`; 0 when `d` is none.
fn along(a: [f64; 2], d: [f64; 2], p: [f64; 2]) -> f64 {
    let len2 = d[0] * d[0] + d[1] * d[1];
    if len2 > 0.0 {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / len2
    } else {
        0.0
    }
}

pub fn footprint(width: f64) -> f64 {
    width / 2.0 + LINE_PAST
}

/// Decks one zoom coarser, at `z`: half a cell wide or more, simplified to
/// half a cell, widest first within a record.
pub fn fold_decks(children: [&Decks; 4], zoom: Zoom) -> Decks {
    let n = f64::from(TILE_CELLS);
    // The parent's half cell of height, in leaf cells.
    let height_tolerance = f64::from(GROUND.finest_in(zoom)) / 2.0;
    let mut lines = Vec::new();
    for (q, child) in children.iter().enumerate() {
        let [qx, qy] = child_of([0, 0], q).map(|v| v as f64 * n);
        for deck in child.iter() {
            let width = deck.width / 2;
            if f64::from(width) < POINT_SCALE / 2.0 {
                continue;
            }
            let points: Vec<[f64; 2]> = (0..deck.points.len()).map(|i| deck.point(i)).map(|[x, y]| [(x + qx) / 2.0, (y + qy) / 2.0]).collect();
            let keep = simplify(&points, deck.tops, 0.5, height_tolerance);
            let kept: Vec<([f64; 2], i16)> = (0..points.len()).filter(|&i| keep[i]).map(|i| (points[i], deck.tops[i])).collect();
            lines.push((kept, width, deck.kind, deck.form, deck.clear));
        }
    }
    let mut widest: Vec<usize> = (0..lines.len()).collect();
    widest.sort_by_key(|&i| std::cmp::Reverse(lines[i].1));
    let (mut admitted, mut decks, mut points) = (vec![false; lines.len()], 0, 0);
    for i in widest {
        if decks < MAX_DECKS && points + lines[i].0.len() <= MAX_POINTS {
            admitted[i] = true;
            decks += 1;
            points += lines[i].0.len();
        }
    }
    let mut out = Builder::default();
    for (line, _) in lines.into_iter().zip(admitted).filter(|(_, a)| *a) {
        out.push(line.0, line.1, line.2, line.3, line.4);
    }
    debug_assert!(out.fits());
    out.finish()
}

/// Douglas–Peucker over position and height.
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

#[derive(Default)]
pub struct Builder {
    starts: Vec<u32>,
    points: Vec<[i16; 2]>,
    tops: Vec<i16>,
    width: Vec<u8>,
    kind: Vec<u8>,
    form: Vec<u8>,
    clear: Vec<u8>,
}

impl Builder {
    /// A line of fewer than two points, or of no width, is dropped.
    pub fn push(&mut self, points: impl IntoIterator<Item = ([f64; 2], i16)>, width: u8, kind: u8, form: Form, clear: u8) {
        let at = self.points.len();
        for ([x, y], top) in points {
            let q = |v: f64| round(v * POINT_SCALE).clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            self.points.push([q(x), q(y)]);
            self.tops.push(top);
        }
        if self.points.len() - at < 2 || width == 0 {
            self.points.truncate(at);
            self.tops.truncate(at);
            return;
        }
        self.starts.push(at as u32);
        self.width.push(width);
        self.kind.push(kind);
        self.form.push(form.byte());
        self.clear.push(clear);
    }

    pub fn fits(&self) -> bool {
        self.width.len() <= MAX_DECKS && self.points.len() <= MAX_POINTS
    }

    pub fn finish(mut self) -> Decks {
        self.starts.push(self.points.len() as u32);
        let mut boxes = vec![EMPTY; boxes_for(self.points.len())];
        build_boxes(&self.starts, &self.points, &self.width, &mut boxes);
        Decks {
            starts: self.starts.into(),
            points: self.points.into(),
            tops: self.tops.into(),
            width: self.width.into(),
            kind: self.kind.into(),
            form: self.form.into(),
            clear: self.clear.into(),
            boxes: boxes.into(),
        }
    }
}

// decks = [starts, points (flattened), tops, width, kind, form, clear]
pub fn write_branch<'a>(decks: &'a Decks, b: &mut BranchViewImpl<'a>) {
    b.branch(Some(&mut |d| {
        d.u32_array(Some(&decks.starts))
            .i16_array(Some(decks.points.as_flattened()))
            .i16_array(Some(&decks.tops))
            .u8_array(Some(&decks.width))
            .u8_array(Some(&decks.kind))
            .u8_array(Some(&decks.form))
            .u8_array(Some(&decks.clear));
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
}

/// Refused unless the lanes are one tile's decks, consistently sized and within a record's room.
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
    };
    let n = lanes.width.len();
    let shaped = lanes.kind.len() == n
        && lanes.form.len() == n
        && lanes.clear.len() == n
        && starts.len() == n + 1
        && lanes.tops.len() == points.len()
        && n <= MAX_DECKS
        && points.len() <= MAX_POINTS
        && starts.first() == Some(&0)
        && starts.last() == Some(&(points.len() as u32))
        && starts.windows(2).all(|w| w[0] <= w[1])
        && lanes.form.iter().all(|&f| usize::from(f) < Form::ALL.len());
    shaped.then_some(lanes).ok_or_else(malformed)
}

impl Lanes<'_> {
    pub fn to_decks(&self) -> Decks {
        let mut boxes = vec![EMPTY; boxes_for(self.points.len())];
        build_boxes(self.starts, self.points, self.width, &mut boxes);
        Decks {
            starts: self.starts.to_vec().into(),
            points: self.points.to_vec().into(),
            tops: self.tops.to_vec().into(),
            width: self.width.to_vec().into(),
            kind: self.kind.to_vec().into(),
            form: self.form.to_vec().into(),
            clear: self.clear.to_vec().into(),
            boxes: boxes.into(),
        }
    }

    pub fn into_room(&self, room: Room) -> Decks {
        fn put<T: Copy>(dst: &'static mut [T], src: &[T]) -> &'static [T] {
            let dst = &mut dst[..src.len()];
            dst.copy_from_slice(src);
            dst
        }
        let boxes = &mut room.boxes[..boxes_for(self.points.len())];
        build_boxes(self.starts, self.points, self.width, boxes);
        Decks {
            starts: put(room.starts, self.starts).into(),
            points: put(room.points, self.points).into(),
            tops: put(room.tops, self.tops).into(),
            width: put(room.width, self.width).into(),
            kind: put(room.kind, self.kind).into(),
            form: put(room.form, self.form).into(),
            clear: put(room.clear, self.clear).into(),
            boxes: (&*boxes).into(),
        }
    }
}

/// Fills `out`, `boxes_for(points.len())` long: a leaf the segments starting
/// at its points, each widened by its deck's reach; a node its children's union.
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
            let r = (way::reach(f64::from(width[d]) / POINT_SCALE) * POINT_SCALE).ceil() as i32;
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

/// The points whose segment's box holds `at`, in order: the tree walked depth first.
struct Near<'a> {
    boxes: &'a [[i16; 4]],
    at: [f64; 2],
    /// each level's first box, leaves at 0
    level_at: [usize; 8],
    stack: [(u8, u16); 8 * FANOUT],
    len: usize,
    leaf: core::ops::Range<usize>,
    points: usize,
}

impl<'a> Near<'a> {
    fn new(boxes: &'a [[i16; 4]], points: usize, at: [f64; 2]) -> Near<'a> {
        let mut near = Near {
            boxes,
            at,
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

    fn holds(&self, b: [i16; 4]) -> bool {
        f64::from(b[0]) <= self.at[0] && self.at[0] <= f64::from(b[2]) && f64::from(b[1]) <= self.at[1] && self.at[1] <= f64::from(b[3])
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
            if !self.holds(self.boxes[self.level_at[level] + k]) {
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

/// A record's room for decks, each lane `MAX_DECKS` or `MAX_POINTS` long, and `MAX_BOXES` for the boxes.
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
#[path = "deck.test.rs"]
mod test;
