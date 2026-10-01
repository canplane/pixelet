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
//! the transport type its surface shows. A bore is the same line through
//! the ground rather than over it — a tunnel's mouth, where it meets a hillside — its points the way's floor. Where a
//! deck lies is the page's lattice, the ground's (space/pyramid.rs
//! `GROUND`), so a level's page holds its decks in its own cells as it holds
//! its ground. A top is in z16 cells, as the elevation is, at every level.
//!
//! The heights are solved offline over the whole network, structure by
//! structure (pxl-geo-bake deck/); nothing here decides one.

use std::borrow::Cow;

use xpute_core::status::errno::Errno;
use xpute_core::status::error::MarshalError;
use xpute_core::wire::xtp::{BranchCursor, BranchViewImpl, NodeView};

use super::way;
use crate::space::hierarchy::child_of;
use crate::space::pyramid::PAGE_CELLS;

/// A point's coordinate in eighths of its page's cell, from the page's
/// south-west corner: an `i16` reaches four thousand cells either way, far
/// past a page's 512, so a deck's points outside its page are held as
/// exactly as those inside it.
pub const POINT_SCALE: f64 = 8.0;

/// A deck's slab, top to underside, in z16 cells. One cell is about the
/// two metres a road deck's structure is.
pub const THICKNESS: i32 = 1;

/// How far apart a deck's piers stand along its line, in its page's cells:
/// at z16 about the 30 m of a short span, and in a coarser page's own cells
/// as far apart on the screen. A product number, set by choice. A pier is
/// the deck's shape, made where the deck is drawn and nothing the bake
/// decides (issue/20261001-ways-off-the-ground.md); counted from the first
/// point of the line a page carries, so a pier's place along a deck may
/// shift where the deck crosses into another page.
pub const PIER_STEP: f64 = 24.0;

/// The farthest past half its width the client draws a line, in its page's
/// cells: a contract the bake keeps a line's page and its extent by, so how
/// the client covers cells with a line — its own (bake/way.rs) — can change
/// within it and nothing is baked again.
pub const LINE_PAST: f64 = 1.0;

/// A bore's air, floor to roof, in z16 cells: about six metres, a road
/// tunnel's 4.5 to 5 m clearance and the lights and fans over it. A product
/// number, set by choice.
pub const BORE_HEIGHT: i32 = 3;

/// The least ground a bore is drawn under over its air, in z16 cells. The
/// line is the way's floor where the solve put it, at the ground at a portal
/// and under it between; how deep under the ground a bore is drawn is the
/// client's, its floor taken down until its air and this roof fit under the
/// ground, as far as `PORTAL_SLOPE` lets it from a portal. A product number,
/// set by choice.
pub const ROOF: f64 = 1.5;

/// How steeply a bore's drawn floor goes down from a portal, rise over run
/// in its page's cells — a road's 8 % at z16, and gentler in a coarser
/// page's own cells. Short of its depth the bore is open to the sky, the
/// cutting a road drops through to a portal. A product number, set by
/// choice.
pub const PORTAL_SLOPE: f64 = 0.08;

/// How far a bore's drawn floor may lie under its ground, in z16 cells: its
/// air and its roof.
pub const BORE_DEPTH: f64 = BORE_HEIGHT as f64 + ROOF;

/// What a line is to the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Carried over it: a bridge, a viaduct, a ramp.
    Deck,
    /// Cut through it: a tunnel, under a roof of the ground over it.
    Bore,
    /// Built up on it: the embankment a bridge's approach climbs on, solid
    /// from the ground to its top.
    Ramp,
}

impl Form {
    pub fn of(byte: u8) -> Form {
        match byte {
            1 => Form::Bore,
            2 => Form::Ramp,
            _ => Form::Deck,
        }
    }

    pub fn byte(self) -> u8 {
        match self {
            Form::Deck => 0,
            Form::Bore => 1,
            Form::Ramp => 2,
        }
    }

    /// Whether it lies under the ground's top rather than over it.
    pub fn cuts(self) -> bool {
        self == Form::Bore
    }
}

/// A resident record's room for decks and their points (pxl-kernel
/// layer/terrain/fixed.rs). The bake keeps every page within them, and
/// refuses to write one it cannot keep within them. Chosen from the memory
/// a record can spend on decks — 48 KiB, an eighth of what a lane a byte a
/// cell would — not from the data, and each lane a whole number of cache
/// lines. At about one point every five metres of deck, the points are some
/// thirty kilometres of deck under one page.
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
    /// A deck's width, in eighths of the page's cell.
    pub width: Cow<'static, [u8]>,
    /// The transport type a deck's surface shows, as the catalog numbers it
    /// (cell/cube_type.spec.rs `transport`).
    pub kind: Cow<'static, [u8]>,
    /// Each line's `Form`, as its byte.
    pub form: Cow<'static, [u8]>,
    /// How far each point of a bore is from the nearest portal down its
    /// tunnel — where it meets the ground and its way carries on at the
    /// surface — in the page's cells; `NO_PORTAL` on a deck, and on a bore
    /// none of whose ends comes out to the ground.
    pub portal: Cow<'static, [u16]>,
}

/// `Decks::portal` where there is none to measure from.
pub const NO_PORTAL: u16 = u16::MAX;

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
    pub width: u8,
    pub kind: u8,
    pub form: Form,
    pub portal: &'a [u16],
}

/// What a line is over a column — one it covers as a way covers the ground
/// (bake/way.rs): its top there (a bore's, its floor), in z16 cells, its
/// kind and form, whether a pier carries it there, and how far the column
/// is from the nearest portal down its tunnel, in the page's cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub top: f64,
    pub kind: u8,
    pub form: Form,
    pub pier: bool,
    pub portal: f64,
}

/// What a line occupies of one column, in z16 cells, each `[lo, hi)`. The
/// one rule the paint draws a line by and the query meets it by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Occupies {
    /// The way on the ground: where a bridge comes down to its end, or a
    /// tunnel's floor is the ground's top. The column's top cell is its
    /// surface.
    Grade,
    /// A slab under its top, and where a pier carries it, the pier from the
    /// ground to the slab.
    Above { slab: [i32; 2], pier: Option<[i32; 2]> },
    /// Air through the ground from the floor up, as far as the ground
    /// stands; the cell under the floor is the way's surface.
    Through { air: [i32; 2] },
}

impl Over {
    /// Over a column whose ground stands `ground` cells high.
    pub fn occupies(&self, ground: i32) -> Occupies {
        let top = self.top.round() as i32;
        match self.form {
            Form::Deck if top > ground => {
                let lo = (top - THICKNESS).max(ground);
                Occupies::Above {
                    slab: [lo, top],
                    pier: (self.pier && lo > ground).then_some([ground, lo]),
                }
            }
            Form::Ramp if top > ground => Occupies::Above { slab: [ground, top], pier: None },
            Form::Bore => {
                // Taken down until its air and its roof fit under the
                // ground, and no steeper than a cutting from a portal.
                let under = f64::from(ground) - self.top;
                let down = (BORE_DEPTH - under).clamp(0.0, self.portal * PORTAL_SLOPE);
                let floor = (self.top - down).round() as i32;
                if floor < ground {
                    Occupies::Through {
                        air: [floor, (floor + BORE_HEIGHT).min(ground)],
                    }
                } else {
                    Occupies::Grade
                }
            }
            _ => Occupies::Grade,
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
            portal: &self.portal[a..b],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Deck<'_>> {
        (0..self.len()).map(|d| self.deck(d))
    }

    /// Every deck over each point of a grid laid on the page: `size` columns
    /// by rows, point (i, j) at `origin + (i + ½, j + ½) · step` in the
    /// page's cells — its own cells' centers at a step of one, and finer
    /// than its cells where a chunk is drawn from a page over its own. Told
    /// as `each(i, j, deck, over)`; a point under two of a deck's segments,
    /// at a joint, is told of both, and the caller keeps the higher.
    pub fn over_grid(&self, origin: [f64; 2], step: f64, size: [i64; 2], mut each: impl FnMut(i64, i64, usize, Over)) {
        let cell = |v: f64, k: usize| (v - origin[k]) / step - 0.5;
        for (d, deck) in self.iter().enumerate() {
            let half = deck.reach();
            let mut along = 0.0;
            for s in 0..deck.points.len().saturating_sub(1) {
                let (a, b) = (deck.point(s), deck.point(s + 1));
                let range = |k: usize| {
                    let (lo, hi) = (cell(a[k].min(b[k]) - half, k).ceil() as i64, cell(a[k].max(b[k]) + half, k).floor() as i64);
                    lo.max(0)..=hi.min(size[k] - 1)
                };
                for j in range(1) {
                    for i in range(0) {
                        let at = [origin[0] + (i as f64 + 0.5) * step, origin[1] + (j as f64 + 0.5) * step];
                        if let Some(over) = deck.over_segment(s, along, at) {
                            each(i, j, d, over);
                        }
                    }
                }
                along += (b[0] - a[0]).hypot(b[1] - a[1]);
            }
        }
    }

    /// Every deck over the point `at`, in the page's cells.
    pub fn over_point(&self, at: [f64; 2]) -> impl Iterator<Item = Over> + '_ {
        self.iter().flat_map(move |deck| {
            let mut along = 0.0;
            (0..deck.points.len().saturating_sub(1)).filter_map(move |s| {
                let over = deck.over_segment(s, along, at);
                let (a, b) = (deck.point(s), deck.point(s + 1));
                along += (b[0] - a[0]).hypot(b[1] - a[1]);
                over
            })
        })
    }

    /// The lowest floor a bore has inside the `[x0, x1) × [y0, y1)` cells,
    /// conservatively, as `highest_in`.
    pub fn lowest_in(&self, [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> Option<i16> {
        let mut low = None;
        for bore in self.iter().filter(|d| d.form.cuts()) {
            let half = bore.footprint();
            for s in 0..bore.points.len().saturating_sub(1) {
                let (a, b) = (bore.point(s), bore.point(s + 1));
                if a[0].max(b[0]) + half < x0 || a[0].min(b[0]) - half > x1 || a[1].max(b[1]) + half < y0 || a[1].min(b[1]) - half > y1 {
                    continue;
                }
                let floor = bore.tops[s].min(bore.tops[s + 1]);
                low = Some(low.map_or(floor, |v: i16| v.min(floor)));
            }
        }
        low
    }

    /// The highest top any deck has inside the `[x0, x1) × [y0, y1)` cells,
    /// conservatively: a deck's highest point counts wherever its line comes
    /// within its footprint of the box.
    pub fn highest_in(&self, [x0, x1]: [f64; 2], [y0, y1]: [f64; 2]) -> Option<i16> {
        let mut top = None;
        for deck in self.iter().filter(|d| !d.form.cuts()) {
            let half = deck.footprint();
            for s in 0..deck.points.len().saturating_sub(1) {
                let (a, b) = (deck.point(s), deck.point(s + 1));
                if a[0].max(b[0]) + half < x0 || a[0].min(b[0]) - half > x1 || a[1].max(b[1]) + half < y0 || a[1].min(b[1]) - half > y1 {
                    continue;
                }
                let t = deck.tops[s].max(deck.tops[s + 1]);
                top = Some(top.map_or(t, |v: i16| v.max(t)));
            }
        }
        top
    }
}

impl Deck<'_> {
    /// Point `i` in the page's cells.
    pub fn point(&self, i: usize) -> [f64; 2] {
        self.points[i].map(|v| f64::from(v) / POINT_SCALE)
    }

    /// Its width in the page's cells.
    pub fn width_cells(&self) -> f64 {
        f64::from(self.width) / POINT_SCALE
    }

    /// How far from its line it covers a cell's center, in the page's cells
    /// (bake/way.rs).
    pub fn reach(&self) -> f64 {
        way::reach(self.width_cells())
    }

    /// How far from its line the client may draw it, in the page's cells:
    /// half its width and `LINE_PAST`.
    pub fn footprint(&self) -> f64 {
        footprint(self.width_cells())
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

    /// What segment `s`, which starts `along` cells down the line, is over
    /// `at`: the top at the nearest point of the segment, if the deck covers
    /// `at`. A pier stands every `PIER_STEP` down the line and takes the cell
    /// it is in, at least, and half the deck's width across; it is told
    /// wherever it stands, and `Over::occupies` raises it only where the
    /// ground is under the slab. A bore is told how far the nearest point
    /// of the segment is from a portal down its tunnel.
    fn over_segment(&self, s: usize, along: f64, at: [f64; 2]) -> Option<Over> {
        let (a, b) = (self.point(s), self.point(s + 1));
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len2 = dx * dx + dy * dy;
        let t = if len2 > 0.0 { (((at[0] - a[0]) * dx + (at[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let near = [a[0] + dx * t, a[1] + dy * t];
        if !way::covers(self.width_cells(), (at[0] - near[0]).hypot(at[1] - near[1])) {
            return None;
        }
        let top = f64::from(self.tops[s]) + (f64::from(self.tops[s + 1]) - f64::from(self.tops[s])) * t;
        let pier_radius = (self.width_cells() / 4.0).max(0.5);
        let down = along + len2.sqrt() * t;
        let off_step = (down - (down / PIER_STEP).round() * PIER_STEP).abs();
        let across = (at[0] - near[0]).hypot(at[1] - near[1]);
        Some(Over {
            top,
            kind: self.kind,
            form: self.form,
            pier: off_step <= pier_radius && across <= pier_radius,
            portal: match (self.portal[s], self.portal[s + 1]) {
                (NO_PORTAL, _) | (_, NO_PORTAL) => f64::INFINITY,
                (p, q) => f64::from(p) + (f64::from(q) - f64::from(p)) * t,
            },
        })
    }
}

/// How far from its line a line `width` cells wide may be drawn.
pub fn footprint(width: f64) -> f64 {
    width / 2.0 + LINE_PAST
}

/// A page's decks one level up, from its four children's: each child's
/// decks in the parent's cells, at their own width, kept while they are as
/// wide as half the parent's cell — by their size and nothing of what they
/// are — and each line simplified to the parent's cell — a point
/// goes where the line without it stays within half a cell of it across and
/// half a cell of height. A deck two children both carry is carried twice,
/// which draws the same columns twice.
pub fn fold_decks(children: [&Decks; 4], level: u32) -> Decks {
    let mut out = Builder::default();
    let n = f64::from(PAGE_CELLS);
    // A z16 cell is 2^level of them under a level-`level` cell; the parent's
    // half cell of height, in z16 cells.
    let height_tolerance = f64::from(1u32 << level) / 2.0;
    for (q, child) in children.iter().enumerate() {
        let [qx, qy] = child_of([0, 0], q).map(|v| v as f64 * n);
        for deck in child.iter() {
            let width = deck.width / 2;
            if f64::from(width) < POINT_SCALE / 2.0 {
                continue;
            }
            let points: Vec<[f64; 2]> = (0..deck.points.len()).map(|i| deck.point(i)).map(|[x, y]| [(x + qx) / 2.0, (y + qy) / 2.0]).collect();
            let keep = simplify(&points, deck.tops, 0.5, height_tolerance);
            let kept: Vec<usize> = (0..points.len()).filter(|&i| keep[i]).collect();
            // A parent's cell is two of the child's.
            let portal = |i: usize| if deck.portal[i] == NO_PORTAL { NO_PORTAL } else { deck.portal[i] / 2 };
            out.push(kept.iter().map(|&i| (points[i], deck.tops[i], portal(i))), width, deck.kind, deck.form);
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
        let len2 = dx * dx + dy * dy;
        let mut worst = (0.0, a);
        for i in a + 1..b {
            let p = points[i];
            let t = if len2 > 0.0 { (((p[0] - pa[0]) * dx + (p[1] - pa[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let off = (p[0] - pa[0] - dx * t).hypot(p[1] - pa[1] - dy * t) / across;
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
    width: Vec<u8>,
    kind: Vec<u8>,
    form: Vec<u8>,
    portal: Vec<u16>,
}

impl Builder {
    /// A deck from its points in the page's cells, each with its top and
    /// how far it is from a portal (`Decks::portal`). A line of fewer than
    /// two points, or of no width, is no deck.
    pub fn push(&mut self, points: impl IntoIterator<Item = ([f64; 2], i16, u16)>, width: u8, kind: u8, form: Form) {
        let at = self.points.len();
        for ([x, y], top, portal) in points {
            let q = |v: f64| (v * POINT_SCALE).round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
            self.points.push([q(x), q(y)]);
            self.tops.push(top);
            self.portal.push(portal);
        }
        if self.points.len() - at < 2 || width == 0 {
            self.points.truncate(at);
            self.tops.truncate(at);
            self.portal.truncate(at);
            return;
        }
        self.starts.push(at as u32);
        self.width.push(width);
        self.kind.push(kind);
        self.form.push(form.byte());
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
            width: self.width.into(),
            kind: self.kind.into(),
            form: self.form.into(),
            portal: self.portal.into(),
        }
    }
}

// ---- The wire ----

// decks = [starts, points, tops, width, kind, form, portal], the points
// flattened.

/// A page's decks as its packet's branch.
pub fn write_branch<'a>(decks: &'a Decks, b: &mut BranchViewImpl<'a>) {
    b.branch(Some(&mut |d| {
        d.u32_array(Some(&decks.starts))
            .i16_array(Some(decks.points.as_flattened()))
            .i16_array(Some(&decks.tops))
            .u8_array(Some(&decks.width))
            .u8_array(Some(&decks.kind))
            .u8_array(Some(&decks.form))
            .u16_array(Some(&decks.portal));
    }));
}

/// A packet's decks, where they lie.
pub struct Lanes<'a> {
    starts: &'a [u32],
    points: &'a [[i16; 2]],
    tops: &'a [i16],
    width: &'a [u8],
    kind: &'a [u8],
    form: &'a [u8],
    portal: &'a [u16],
}

/// The branch's lanes: refused unless they are one page's decks — as many
/// widths, kinds and forms as decks, as many tops and portals as points, the starts
/// rising from the first point to past the last, and no more than a record
/// holds.
pub fn read_branch(d: BranchCursor<'_>) -> Result<Lanes<'_>, MarshalError> {
    let bad = || MarshalError::new(Errno::EBADMSG);
    let starts = d.at(0)?.get_array::<u32>()?;
    let points = match d.at(1)?.get_array::<i16>()?.as_chunks::<2>() {
        (points, []) => points,
        _ => return Err(bad()),
    };
    let lanes = Lanes {
        starts,
        points,
        tops: d.at(2)?.get_array::<i16>()?,
        width: d.at(3)?.get_array::<u8>()?,
        kind: d.at(4)?.get_array::<u8>()?,
        form: d.at(5)?.get_array::<u8>()?,
        portal: d.at(6)?.get_array::<u16>()?,
    };
    let n = lanes.width.len();
    let shaped = lanes.kind.len() == n
        && lanes.form.len() == n
        && lanes.portal.len() == points.len()
        && starts.len() == n + 1
        && lanes.tops.len() == points.len()
        && n <= MAX_DECKS
        && points.len() <= MAX_POINTS
        && starts.first() == Some(&0)
        && starts.last() == Some(&(points.len() as u32))
        && starts.windows(2).all(|w| w[0] <= w[1]);
    shaped.then_some(lanes).ok_or_else(bad)
}

impl Lanes<'_> {
    /// Copied out, for a page held in hand.
    pub fn to_decks(&self) -> Decks {
        Decks {
            starts: self.starts.to_vec().into(),
            points: self.points.to_vec().into(),
            tops: self.tops.to_vec().into(),
            width: self.width.to_vec().into(),
            kind: self.kind.to_vec().into(),
            form: self.form.to_vec().into(),
            portal: self.portal.to_vec().into(),
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
            width: put(room.width, self.width).into(),
            kind: put(room.kind, self.kind).into(),
            form: put(room.form, self.form).into(),
            portal: put(room.portal, self.portal).into(),
        }
    }
}

/// A record's room for decks, each lane as long as a record holds
/// (`MAX_DECKS`, `MAX_POINTS`).
pub struct Room {
    pub starts: &'static mut [u32],
    pub points: &'static mut [[i16; 2]],
    pub tops: &'static mut [i16],
    pub width: &'static mut [u8],
    pub kind: &'static mut [u8],
    pub form: &'static mut [u8],
    pub portal: &'static mut [u16],
}

#[cfg(test)]
#[path = "deck.test.rs"]
mod test;
