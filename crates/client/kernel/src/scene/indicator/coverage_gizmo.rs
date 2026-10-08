// pxl-kernel/scene/indicator/coverage_gizmo.rs

//! The coverage gizmo (the `world_gizmo` setting), as translucent debug lines: the
//! drawn pieces, the goal's pieces nothing drawn covers and the drawn pieces'
//! tiles, each colored as its `Mark` says.
//! Every line is cut at each edge's ends on it, and each stretch drawn once in
//! the highest mark over it, straight between its ends on the ground:
//! overlaid, the lines read as none of them, and laid along the ground they
//! wavered with every cell they crossed.

use crate::layer::{Answer, Query};
use crate::view::demand::Observed as _;
use std::collections::{HashMap, HashSet};

use pxl_render::frame::draw::DrawList;
use pxl_spec::bake::format::TerrainTile;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::chunk::Chunk;
use crate::layer::residency::floor;
use crate::layer::Base as _;
use crate::query::World;
use crate::scene::indicator::debug_lines::DebugLines;
use crate::view::frame::pose::view;
use crate::view::observation::observation;
use pxl_spec::space::coord::zoom::{units, within, Zoom};

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

#[derive(Default)]
pub struct CoverageGizmo {
    lines: DebugLines,
    /// What is drawn, and about where; none while hidden.
    drawn: Option<V3>,
    lane: Vec<f32>,
    /// The lines laid this step, kept so a step allocates nothing.
    next: Vec<f32>,
}

impl CoverageGizmo {
    /// `drawn` sorted.
    pub fn step(&mut self, draw: &mut DrawList, show: bool, goal: &[Chunk], drawn: &[Chunk]) {
        if !show {
            if self.drawn.take().is_some() {
                self.lines.set(draw, [0.0; 3], &[]);
            }
            return;
        }
        let v = view();
        // Its lines are f32 about the corner under the eye of a tile about as wide as
        // the eye is high: what is near resolves, and a move rebuilds them once a height.
        let height = observation().map_or(0.0, |o| o.eye[2] - o.ground).max(1.0);
        let zoom = within(height).clamp(i32::from(GROUND.top()), i32::from(LEAF_ZOOM)) as Zoom;
        let span = units(zoom);
        let about = v.at().eye.map(|c| (c / span).floor() * span);
        let about = [about[0], about[1], 0.0];
        let residency = TerrainTile::residency();
        let top = floor::top().unwrap_or(LEAF_ZOOM);
        let lined: HashSet<Tile> = TerrainTile::demand().demand().iter().map(|&p| line_top(p, top)).collect();
        let floor_only = |p: Tile| p.zoom <= top && !lined.contains(&p);
        let over: HashSet<Chunk> = drawn.iter().flat_map(|d| d.ancestors()).collect();
        let mut lines = Lines::default();
        for &g in goal {
            let covered = drawn.binary_search(&g).is_ok() || over.contains(&g) || g.ancestors().any(|a| drawn.binary_search(&a).is_ok());
            if covered {
                continue;
            }
            let mark = match residency.read(g.node(), g.cell_zoom()).value() {
                None => Mark::NoTile,
                Some(s) if floor_only(s.from) => Mark::FloorOnly,
                Some(_) => Mark::Unbuilt,
            };
            lines.square(g.rect(), mark);
        }
        let mut tiles: Vec<Tile> = Vec::with_capacity(drawn.len());
        for &d in drawn {
            let tile = GROUND.read_tile(d.column(), d.cell_zoom());
            let from_floor = matches!(residency.read(d.node(), d.cell_zoom()), Answer::Coarser(s, _) if floor_only(s.from));
            lines.square(d.rect(), if from_floor { Mark::DrawnFromFloor } else { Mark::Drawn });
            tiles.push(tile);
        }
        tiles.sort_unstable();
        tiles.dedup();
        for p in tiles {
            lines.square(p.rect(), Mark::Tile);
        }
        self.next.clear();
        lines.lay(&mut self.next, about, &World::now());
        if self.drawn != Some(about) || self.next != self.lane {
            self.lines.set(draw, about, &self.next);
            self.drawn = Some(about);
            core::mem::swap(&mut self.next, &mut self.lane);
        }
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
    fn lay(&self, lane: &mut Vec<f32>, origin: V3, access: &World) {
        let ground = |x: f64, y: f64| -> V3 {
            let z = access.surface_support(&world::point(x, y, 0.0)).value().flatten().unwrap_or(0.0);
            [pxl_spec::space::coord::world::nearest(x, origin[0]) - origin[0], y - origin[1], z - origin[2]]
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

fn stretch(lane: &mut Vec<f32>, a: V3, b: V3, mark: Mark) {
    let pieces = if mark.dashed() { DASHES } else { 1 };
    let at = |t: u32| -> [f32; 3] { core::array::from_fn(|i| (a[i] + (b[i] - a[i]) * t as f64 / pieces as f64) as f32) };
    for i in (0..pieces).step_by(2) {
        for p in [at(i), at(i + 1)] {
            lane.extend_from_slice(&p);
            lane.extend_from_slice(&mark.rgba());
        }
    }
}
