// pxl-geo-bake/deck/relate.rs

//! What touches, crosses or lies under the pieces, from rows already read.

use std::collections::HashMap;

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::space::adapter::epsg3857::to_db;
use pxl_spec::space::pyramid::PAGE_CELLS;

use crate::deck::piece::{BridgeRow, Piece};
use crate::wkb::wkb_to_geometries;

/// A way within this of a point, in DB coordinates, passes through or ends there.
pub const TOUCH_REACH: f64 = 0.5;

/// A step along a way from a point, in DB coordinates.
pub const SIDE_STEP: f64 = 0.5;

type P = [f64; 2];

/// A polyline with its length so far at each point.
pub struct Polyline {
    pub points: Vec<P>,
    along: Vec<f64>,
}

impl Polyline {
    pub fn of(points: impl IntoIterator<Item = P>) -> Polyline {
        let points: Vec<P> = points.into_iter().collect();
        let mut along = vec![0.0; points.len()];
        for i in 1..points.len() {
            along[i] = along[i - 1] + libm::hypot(points[i][0] - points[i - 1][0], points[i][1] - points[i - 1][1]);
        }
        Polyline { points, along }
    }

    pub fn length(&self) -> f64 {
        self.along.last().copied().unwrap_or(0.0)
    }

    fn segments(&self) -> impl Iterator<Item = (usize, P, P)> + '_ {
        self.points.windows(2).enumerate().map(|(i, w)| (i, w[0], w[1]))
    }

    /// The nearest point's distance and where it is along, in DB coordinates.
    fn nearest(&self, p: P) -> (f64, f64) {
        let mut best = (f64::INFINITY, 0.0);
        for (i, a, b) in self.segments() {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let d = libm::hypot(p[0] - a[0] - dx * t, p[1] - a[1] - dy * t);
            if d < best.0 {
                best = (d, self.along[i] + t * libm::sqrt(len2));
            }
        }
        best
    }
}

/// A way as read: its row and its line.
pub struct Way {
    pub row: BridgeRow,
    pub line: Polyline,
}

impl Way {
    pub fn of(row: BridgeRow) -> Option<Way> {
        let g = wkb_to_geometries(&row.geom_wkb).into_iter().next()?;
        (g.points.len() >= 2).then(|| Way { line: Polyline::of(g.points.iter().map(|p| [p[0], p[1]])), row })
    }
}

/// A water body as read: every ring of every polygon, holes alike.
pub struct Water {
    pub subtype: String,
    pub class: Option<String>,
    rings: Vec<Vec<P>>,
}

impl Water {
    pub fn of(subtype: String, class: Option<String>, wkb: &[u8]) -> Water {
        let rings = wkb_to_geometries(wkb)
            .into_iter()
            .flat_map(|g| std::iter::once(g.points).chain(g.holes))
            .map(|r| r.iter().map(|p| [p[0], p[1]]).collect())
            .collect();
        Water { subtype, class, rings }
    }

    fn edges(&self) -> impl Iterator<Item = (P, P)> + '_ {
        self.rings.iter().flat_map(|r| r.windows(2).map(|w| (w[0], w[1])))
    }

    /// Even-odd, so holes are out.
    fn holds(&self, p: P) -> bool {
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
        }
        inside
    }
}

/// Where `a`–`b` and `c`–`d` meet, as fractions along each; parallel ones never.
fn meet(a: P, b: P, c: P, d: P) -> Option<(f64, f64)> {
    let (r, s) = ([b[0] - a[0], b[1] - a[1]], [d[0] - c[0], d[1] - c[1]]);
    let den = r[0] * s[1] - r[1] * s[0];
    if den == 0.0 {
        return None;
    }
    let q = [c[0] - a[0], c[1] - a[1]];
    let (t, u) = ((q[0] * s[1] - q[1] * s[0]) / den, (q[0] * r[1] - q[1] * r[0]) / den);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some((t, u))
}

/// Segments by the cells of a page's side they span.
struct Grid {
    cells: HashMap<(i64, i64), Vec<(usize, usize)>>,
}

fn cells_of(a: P, b: P, reach: f64) -> impl Iterator<Item = (i64, i64)> {
    let cell = to_db(f64::from(PAGE_CELLS) * U_PER_CELL);
    let c = move |v: f64| libm::floor(v / cell) as i64;
    let (x0, x1) = (c(a[0].min(b[0]) - reach), c(a[0].max(b[0]) + reach));
    let (y0, y1) = (c(a[1].min(b[1]) - reach), c(a[1].max(b[1]) + reach));
    (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
}

impl Grid {
    fn of<'a>(lines: impl Iterator<Item = &'a Polyline>) -> Grid {
        let mut cells: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
        for (l, line) in lines.enumerate() {
            for (s, a, b) in line.segments() {
                for c in cells_of(a, b, 0.0) {
                    cells.entry(c).or_default().push((l, s));
                }
            }
        }
        Grid { cells }
    }

    /// Each line and segment near `a`–`b` once.
    fn near(&self, a: P, b: P, reach: f64) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = cells_of(a, b, reach).filter_map(|c| self.cells.get(&c)).flatten().copied().collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

pub struct WayCrossing {
    pub piece: usize,
    pub gers_id: String,
    pub at: f64,
    pub subtype: String,
    pub class: Option<String>,
    pub flags: Option<String>,
    pub own_at: f64,
}

/// Every way meeting a piece: where along the piece, and where along the way.
pub fn ways_crossing(pieces: &[Polyline], ways: &[Way]) -> Vec<WayCrossing> {
    let grid = Grid::of(ways.iter().map(|w| &w.line));
    let mut out = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        for (s, a, b) in piece.segments() {
            for (w, ws) in grid.near(a, b, 0.0) {
                let way = &ways[w];
                let (c, d) = (way.line.points[ws], way.line.points[ws + 1]);
                let Some((t, u)) = meet(a, b, c, d) else { continue };
                out.push(WayCrossing {
                    piece: i,
                    gers_id: way.row.gers_id.clone(),
                    at: piece.along[s] + t * (piece.along[s + 1] - piece.along[s]),
                    subtype: way.row.subtype.clone(),
                    class: way.row.class.clone(),
                    flags: way.row.flags.clone(),
                    own_at: (way.line.along[ws] + u * (way.line.along[ws + 1] - way.line.along[ws])) / way.line.length().max(f64::EPSILON),
                });
            }
        }
    }
    out
}

pub struct WaterUnder {
    pub piece: usize,
    pub from: f64,
    pub to: f64,
    pub subtype: String,
    pub class: Option<String>,
}

/// The stretches of each piece over water, along it in DB coordinates.
pub fn water_under(pieces: &[Polyline], water: &[Water]) -> Vec<WaterUnder> {
    let rings: Vec<Polyline> = water.iter().flat_map(|w| w.rings.iter().map(|r| Polyline::of(r.iter().copied()))).collect();
    let owner: Vec<usize> = water.iter().enumerate().flat_map(|(k, w)| std::iter::repeat_n(k, w.rings.len())).collect();
    let grid = Grid::of(rings.iter());
    let mut out = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        let mut cuts: HashMap<usize, Vec<f64>> = HashMap::new();
        for (s, a, b) in piece.segments() {
            for (r, rs) in grid.near(a, b, 0.0) {
                let (c, d) = (rings[r].points[rs], rings[r].points[rs + 1]);
                if let Some((t, _)) = meet(a, b, c, d) {
                    cuts.entry(owner[r]).or_default().push(piece.along[s] + t * (piece.along[s + 1] - piece.along[s]));
                }
            }
        }
        let mut bodies: Vec<usize> = cuts.keys().copied().collect();
        bodies.sort_unstable();
        for k in bodies {
            let mut at = cuts.remove(&k).unwrap_or_default();
            at.extend([0.0, piece.length()]);
            at.sort_by(f64::total_cmp);
            at.dedup();
            for w in at.windows(2) {
                if water[k].holds(point_at(piece, (w[0] + w[1]) / 2.0)) {
                    out.push(WaterUnder { piece: i, from: w[0], to: w[1], subtype: water[k].subtype.clone(), class: water[k].class.clone() });
                }
            }
        }
    }
    out
}

fn point_at(line: &Polyline, d: f64) -> P {
    let i = line.along.partition_point(|&x| x < d).clamp(1, line.points.len() - 1);
    let (a, b) = (line.points[i - 1], line.points[i]);
    let seg = line.along[i] - line.along[i - 1];
    let t = if seg > 0.0 { (d - line.along[i - 1]) / seg } else { 0.0 };
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// Two pieces crossing through each other, not meeting at an end: `upper`
/// over `lower` by `layer`, ties by id, so the order is one total order.
pub struct DeckCrossing {
    pub upper: usize,
    pub upper_at: f64,
    pub lower: usize,
    pub lower_at: f64,
    pub tied: bool,
}

pub fn decks_crossing(pieces: &[&Piece], lines: &[Polyline]) -> Vec<DeckCrossing> {
    let grid = Grid::of(lines.iter());
    let at_end = |line: &Polyline, s: usize, t: f64| (s == 0 && t == 0.0) || (s + 2 == line.points.len() && t == 1.0);
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        for (s, a, b) in line.segments() {
            for (j, js) in grid.near(a, b, 0.0) {
                let order = |k: usize| (pieces[k].level, pieces[k].gers_id.as_str());
                if order(j) >= order(i) {
                    continue;
                }
                let other = &lines[j];
                let Some((t, u)) = meet(a, b, other.points[js], other.points[js + 1]) else { continue };
                if at_end(line, s, t) || at_end(other, js, u) {
                    continue;
                }
                out.push(DeckCrossing {
                    upper: i,
                    upper_at: line.along[s] + t * (line.along[s + 1] - line.along[s]),
                    lower: j,
                    lower_at: other.along[js] + u * (other.along[js + 1] - other.along[js]),
                    tied: pieces[i].level == pieces[j].level,
                });
            }
        }
    }
    out
}

/// Ways connected at each point (ending there, or the point's own way), not
/// ones passing over or under, and where along each the point is.
pub fn connected<'a>(points: &[(P, &str)], ways: &'a [Way]) -> Vec<(usize, f64, &'a Way)> {
    let grid = Grid::of(ways.iter().map(|w| &w.line));
    let mut out = Vec::new();
    for (k, &(p, own)) in points.iter().enumerate() {
        let mut seen: Vec<usize> = grid.near(p, p, TOUCH_REACH).into_iter().map(|(w, _)| w).collect();
        seen.dedup();
        for w in seen {
            let way = &ways[w];
            let (d, along) = way.line.nearest(p);
            let ends = [way.line.points[0], way.line.points[way.line.points.len() - 1]];
            let at_end = ends.iter().any(|e| libm::hypot(e[0] - p[0], e[1] - p[1]) <= TOUCH_REACH);
            if d <= TOUCH_REACH && (way.row.gers_id == own || at_end) {
                out.push((k, along / way.line.length().max(f64::EPSILON), way));
            }
        }
    }
    out
}
