// pxl-kernel/layer/marker/coverage_gizmo.rs

//! The coverage gizmo (the `gizmos` setting), as translucent debug lines: the
//! drawn pieces, the goal's pieces nothing drawn covers and the drawn pieces'
//! tiles, each colored as its `Mark` says.
//! Every line is cut at each edge's ends on it, and each stretch drawn once in
//! the highest mark over it, straight between its ends on the ground:
//! overlaid, the lines read as none of them, and laid along the ground they
//! wavered with every cell they crossed.

use std::collections::{HashMap, HashSet};

use pxl_render::frame::draw::DrawList;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pvf3::projection;
use pxl_spec::space::pyramid::{Tile, LEAF_Z, TOP_Z};
use pxl_spec::space::v3::V3;

use crate::layer::marker::debug_lines::DebugLines;
use crate::world::access::Access;
use crate::world::frame::view::view;
use crate::world::observation::observation;
use crate::world::pyramid::chunk::Chunk;
use crate::world::pyramid::Layer as _;
use crate::world::residency::floor;
use pxl_spec::space::zoom::{units, within, Zoom};

/// What an edge marks, lowest first: a stretch takes the highest over it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mark {
    /// White, solid: a drawn piece.
    Drawn,
    /// Cyan, solid: a drawn piece read from the floor's tile alone.
    DrawnFromFloor,
    /// Yellow, dashed: a goal piece with a tile on its line, its mesh not yet built.
    Unbuilt,
    /// Magenta, dashed: a goal piece only the floor's tile can build.
    FloorOnly,
    /// Red, dashed: a goal piece with no tile on its line.
    NoTile,
    /// Orange: a drawn piece's tile.
    Tile,
}

const MARKS: usize = Mark::ALL.len();

impl Mark {
    const ALL: [Mark; 6] = [Mark::Drawn, Mark::DrawnFromFloor, Mark::Unbuilt, Mark::FloorOnly, Mark::NoTile, Mark::Tile];

    fn rgba(self) -> [f32; 4] {
        match self {
            Mark::Drawn => [1.0, 1.0, 1.0, 0.4],
            Mark::DrawnFromFloor => [0.0, 1.0, 1.0, 0.6],
            Mark::Unbuilt => [1.0, 1.0, 0.0, 0.6],
            Mark::FloorOnly => [1.0, 0.0, 1.0, 0.6],
            Mark::NoTile => [1.0, 0.0, 0.0, 0.6],
            Mark::Tile => [1.0, 0.3, 0.0, 0.9],
        }
    }

    fn dashed(self) -> bool {
        matches!(self, Mark::Unbuilt | Mark::FloorOnly | Mark::NoTile)
    }
}

/// Dashes along a dashed stretch, every other one drawn.
const DASHES: u32 = 16;

/// What the lines were drawn for: the drawn set's, the goal's and the tiles'
/// revisions, and the origin.
type Seen = (u32, u32, u32, [u64; 3]);

#[derive(Default)]
pub struct CoverageGizmo {
    lines: DebugLines,
    lane: Vec<f32>,
    seen: Option<Seen>,
}

impl CoverageGizmo {
    /// `drawn` sorted.
    pub fn step(&mut self, draw: &mut DrawList, show: bool, revisions: (u32, u32, u32), goal: &[Chunk], drawn: &[Chunk]) {
        if !show {
            if self.seen.take().is_some() {
                self.lines.set(draw, [0.0; 3], &[]);
            }
            return;
        }
        let v = view();
        // Its lines are f32 about the corner under the eye of a tile about as wide as
        // the eye is high: what is near resolves, and a move rebuilds them once a height.
        let height = observation().map_or(0.0, |o| (o.eye[2] - o.ground).max(1.0));
        let zoom = within(height).clamp(i32::from(TOP_Z), i32::from(LEAF_Z)) as Zoom;
        let span = units(zoom);
        let about = v.pose.eye.map(|c| (c / span).floor() * span);
        let about = [about[0], about[1], 0.0];
        let seen = (revisions.0, revisions.1, revisions.2, about.map(f64::to_bits));
        if self.seen == Some(seen) {
            return;
        }
        self.seen = Some(seen);
        let residency = Terrain::pyramid().residency();
        let top = floor::top().unwrap_or(LEAF_Z);
        let lined: HashSet<Tile> = Terrain::pyramid().demand().iter().map(|&p| line_top(p, top)).collect();
        let floor_only = |p: Tile| p.zoom <= top && !lined.contains(&p);
        let over: HashSet<Chunk> = drawn.iter().flat_map(|d| d.ancestors()).collect();
        let mut lines = Lines::default();
        for &g in goal {
            let covered = drawn.binary_search(&g).is_ok() || over.contains(&g) || g.ancestors().any(|a| drawn.binary_search(&a).is_ok());
            if covered {
                continue;
            }
            let mark = match residency.drawn_from(g.tile()) {
                None => Mark::NoTile,
                Some(src) if floor_only(src) => Mark::FloorOnly,
                Some(_) => Mark::Unbuilt,
            };
            lines.square(g.rect(), mark);
        }
        let mut tiles: Vec<Tile> = Vec::with_capacity(drawn.len());
        for &d in drawn {
            let from_floor = residency.source(d.tile()).is_none() && residency.drawn_from(d.tile()).is_some_and(floor_only);
            lines.square(d.rect(), if from_floor { Mark::DrawnFromFloor } else { Mark::Drawn });
            tiles.push(d.tile());
        }
        tiles.sort_unstable();
        tiles.dedup();
        for p in tiles {
            lines.square(p.rect(), Mark::Tile);
        }
        self.lane.clear();
        lines.lay(&mut self.lane, about, &Access::now());
        self.lines.set(draw, about, &self.lane);
    }

    pub fn draw(&self, draw: &mut DrawList, eye: V3) {
        self.lines.draw(draw, eye);
    }
}

fn line_top(mut p: Tile, top: Zoom) -> Tile {
    while p.zoom > top {
        p = p.parent();
    }
    p
}

/// Edges by the line they lie on, its axis (0 along x, 1 along y) and where
/// it crosses the other: each a stretch along it and its mark.
#[derive(Default)]
struct Lines(HashMap<(u8, u64), Vec<Stretch>>);

/// From, to, and what the edge marks.
type Stretch = (f64, f64, Mark);

impl Lines {
    /// `rect` gives the south-west corner and the side.
    fn square(&mut self, (x0, y0, side): (f64, f64, f64), mark: Mark) {
        let (x1, y1) = (x0 + side, y0 + side);
        for (axis, at, from, to) in [(0, y0, x0, x1), (0, y1, x0, x1), (1, x0, y0, y1), (1, x1, y0, y1)] {
            self.0.entry((axis, at.to_bits())).or_default().push((from, to, mark));
        }
    }

    /// Each line swept once, cut at every edge's end on it: a stretch takes
    /// the highest mark over it and is drawn straight between its ends on the
    /// ground.
    fn lay(&self, lane: &mut Vec<f32>, origin: V3, access: &Access) {
        let ground = |x: f64, y: f64| -> [f64; 3] {
            let z = access.surface_support(&projection::project(x, y, 0.0)).value().flatten().unwrap_or(0.0);
            [pxl_spec::space::pvf3::nearest(x, origin[0]) - origin[0], y - origin[1], z - origin[2]]
        };
        let mut ends: Vec<(f64, bool, Mark)> = Vec::new();
        for (&(axis, at), edges) in &self.0 {
            let at = f64::from_bits(at);
            let point = |t: f64| if axis == 0 { ground(t, at) } else { ground(at, t) };
            ends.clear();
            ends.extend(edges.iter().flat_map(|&(a, b, m)| [(a, true, m), (b, false, m)]));
            ends.sort_unstable_by(|x, y| x.0.total_cmp(&y.0));
            let mut over = [0u32; MARKS];
            let mut from: Option<f64> = None;
            let mut k = 0;
            while k < ends.len() {
                let t = ends[k].0;
                // Every edge on the line cuts it, so a coarse edge over finer
                // ones is drawn as they are, in its own mark.
                if let (Some(f), Some(m)) = (from, (0..MARKS).rev().find(|&m| over[m] > 0)) {
                    stretch(lane, point(f), point(t), Mark::ALL[m]);
                }
                while k < ends.len() && ends[k].0 == t {
                    let (_, opens, m) = ends[k];
                    if opens {
                        over[m as usize] += 1;
                    } else {
                        over[m as usize] -= 1;
                    }
                    k += 1;
                }
                from = Some(t);
            }
        }
    }
}

fn stretch(lane: &mut Vec<f32>, a: [f64; 3], b: [f64; 3], mark: Mark) {
    let pieces = if mark.dashed() { DASHES } else { 1 };
    let at = |t: u32| -> [f32; 3] { core::array::from_fn(|i| (a[i] + (b[i] - a[i]) * t as f64 / pieces as f64) as f32) };
    for i in (0..pieces).step_by(2) {
        for p in [at(i), at(i + 1)] {
            lane.extend_from_slice(&p);
            lane.extend_from_slice(&mark.rgba());
        }
    }
}
