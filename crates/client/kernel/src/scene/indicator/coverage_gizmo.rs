// pxl-kernel/scene/indicator/coverage_gizmo.rs

//! The coverage gizmo (the `world_gizmo` setting), as translucent debug lines: the
//! drawn pieces, the goal's pieces nothing drawn covers, the drawn pieces'
//! tiles and their shards, each colored as its `Mark` says.
//! Every line is cut at each edge's ends on it, and each stretch drawn once in
//! the highest mark over it, straight between its ends on the ground:
//! overlaid, the lines read as none of them, and laid along the ground they
//! wavered with every cell they crossed.

use crate::layer::{Answer, Query};
use crate::view::demand::Observed as _;
use std::collections::{HashMap, HashSet};

use pxl_render::frame::draw::DrawList;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::chunk::Chunk;
use crate::layer::residency::floor;
use crate::layer::Base as _;
use crate::query::World;
use crate::scene::indicator::debug_lines::DebugLines;
use crate::scene::indicator::gizmo_spec;
use crate::view::frame::pose::view;
use crate::view::observation::observation;
use pxl_render::look::color::linear_rgb;
use pxl_spec::space::coord::zoom::{units, within, Zoom};

/// What an edge marks. Its look and its rank are the spec's
/// (`gizmo_spec::coverage`): a stretch takes the mark of highest rank over it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Drawn,
    DrawnFromFloor,
    Unbuilt,
    FloorOnly,
    NoTile,
    Tile,
    Shard,
}

const MARKS: usize = gizmo_spec::coverage::KEYS.len();

impl Mark {
    const ALL: [Mark; MARKS] = [Mark::Drawn, Mark::DrawnFromFloor, Mark::Unbuilt, Mark::FloorOnly, Mark::NoTile, Mark::Tile, Mark::Shard];

    /// Its name in the spec, its color, opacity and dash.
    fn look(self) -> (&'static str, u32, f32, bool) {
        use gizmo_spec::coverage::*;
        match self {
            Mark::Drawn => ("drawn", drawn::rgb, drawn::alpha, drawn::dashed),
            Mark::DrawnFromFloor => ("drawn_from_floor", drawn_from_floor::rgb, drawn_from_floor::alpha, drawn_from_floor::dashed),
            Mark::Unbuilt => ("unbuilt", unbuilt::rgb, unbuilt::alpha, unbuilt::dashed),
            Mark::FloorOnly => ("floor_only", floor_only::rgb, floor_only::alpha, floor_only::dashed),
            Mark::NoTile => ("no_tile", no_tile::rgb, no_tile::alpha, no_tile::dashed),
            Mark::Tile => ("tile", tile::rgb, tile::alpha, tile::dashed),
            Mark::Shard => ("shard", shard::rgb, shard::alpha, shard::dashed),
        }
    }

    /// Its place in the spec's order.
    fn rank(self) -> usize {
        gizmo_spec::coverage::KEYS.iter().position(|&k| k == self.look().0).expect("a mark the spec names")
    }

    fn of_rank(rank: usize) -> Mark {
        *Mark::ALL.iter().find(|m| m.rank() == rank).expect("a rank the spec has")
    }

    fn rgba(self) -> [f32; 4] {
        let (_, rgb, a, _) = self.look();
        let [r, g, b] = linear_rgb(rgb).map(|c| c as f32);
        [r, g, b, a]
    }

    fn dashed(self) -> bool {
        self.look().3
    }
}

/// Dashes along a dashed stretch, every other one drawn.
const DASHES: u32 = 16;

#[derive(Default)]
pub struct CoverageGizmo {
    lines: DebugLines,
    /// What is drawn, and about where; none while hidden.
    drawn: Option<V3>,
    /// The lines then the faces, and where the faces begin.
    lane: (Vec<f32>, usize),
    /// Those laid this step, kept so their buffer is reused.
    next: (Vec<f32>, usize),
}

impl CoverageGizmo {
    /// `drawn` sorted.
    pub fn step(&mut self, draw: &mut DrawList, show: bool, goal: &[Chunk], drawn: &[Chunk]) {
        if !show {
            if self.drawn.take().is_some() {
                self.lines.set(draw, [0.0; 3], &[], 0);
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
            lines.piece(g.rect(), mark);
        }
        let mut tiles: Vec<Tile> = Vec::with_capacity(drawn.len());
        let mut shards: Vec<Chunk> = Vec::with_capacity(drawn.len());
        for &d in drawn {
            shards.push(d.ancestors().last().unwrap_or(d));
            let tile = GROUND.read_tile(d.column(), d.cell_zoom());
            let from_floor = matches!(residency.read(d.node(), d.cell_zoom()), Answer::Coarser(s, _) if floor_only(s.from));
            lines.piece(d.rect(), if from_floor { Mark::DrawnFromFloor } else { Mark::Drawn });
            tiles.push(tile);
        }
        tiles.sort_unstable();
        tiles.dedup();
        for p in tiles {
            lines.square(p.rect(), Mark::Tile);
        }
        shards.sort_unstable();
        shards.dedup();
        for s in shards {
            lines.square(s.rect(), Mark::Shard);
        }
        self.next.0.clear();
        self.next.1 = lines.lay(&mut self.next.0, about, &World::now());
        if self.drawn != Some(about) || self.next != self.lane {
            self.lines.set(draw, about, &self.next.0, self.next.1);
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
/// And the pieces' squares, whose faces hide what of the gizmo is behind them.
#[derive(Default)]
struct Lines(HashMap<(u8, u64), Vec<Stretch>>, Vec<(f64, f64, f64)>);

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

    /// A piece's square, its face among them.
    fn piece(&mut self, rect: (f64, f64, f64), mark: Mark) {
        self.square(rect, mark);
        self.1.push(rect);
    }

    /// Each line swept once, cut at every edge's end on it: a stretch takes
    /// the highest mark over it and is drawn straight between its ends on the
    /// ground. Then each piece's face, a fan from its center through every cut
    /// on its edges, so its rim is the lines' own. Returns where the faces begin.
    fn lay(&self, lane: &mut Vec<f32>, origin: V3, access: &World) -> usize {
        let mut grounds: HashMap<(u64, u64), V3> = HashMap::new();
        let mut ends: Vec<(f64, bool, Mark)> = Vec::new();
        // Each point asked once, by the lines and the faces alike.
        let mut ground = |x: f64, y: f64| -> V3 {
            *grounds.entry((x.to_bits(), y.to_bits())).or_insert_with(|| {
                let z = access.surface_support(&world::point(x, y, 0.0)).value().flatten().unwrap_or(0.0);
                [pxl_spec::space::coord::world::nearest(x, origin[0]) - origin[0], y - origin[1], z - origin[2]]
            })
        };
        for (&(axis, at), edges) in &self.0 {
            let at = f64::from_bits(at);
            let mut point = |t: f64| if axis == 0 { ground(t, at) } else { ground(at, t) };
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
                    stretch(lane, point(f), point(t), Mark::of_rank(m));
                }
                while k < ends.len() && ends[k].0 == t {
                    let (_, opens, m) = ends[k];
                    if opens {
                        over[m.rank()] += 1;
                    } else {
                        over[m.rank()] -= 1;
                    }
                    k += 1;
                }
                from = Some(t);
            }
        }
        let lines = lane.len();
        let mut rim: Vec<f64> = Vec::new();
        // A face only where no finer piece lies inside it, at whatever height: a
        // coarse face would hide the finer edges where the ground dips under it.
        let key = |(x, y, side): (f64, f64, f64)| (x.to_bits(), y.to_bits(), side.to_bits());
        let widest = self.1.iter().map(|r| r.2).fold(0.0, f64::max);
        let mut coarser = HashSet::new();
        for &(x0, y0, side) in &self.1 {
            let mut s = side * 2.0;
            while s <= widest {
                coarser.insert(key(((x0 / s).floor() * s, (y0 / s).floor() * s, s)));
                s *= 2.0;
            }
        }
        let mut laid = HashSet::new();
        for &(x0, y0, side) in self.1.iter().filter(|&&r| !coarser.contains(&key(r)) && laid.insert(key(r))) {
            let (x1, y1) = (x0 + side, y0 + side);
            let center = ground(x0 + side / 2.0, y0 + side / 2.0);
            let mut prev: Option<V3> = None;
            // Counterclockwise from the south-west corner: each edge's cuts, its far corner last.
            for (axis, at, from, to) in [(0, y0, x0, x1), (1, x1, y0, y1), (0, y1, x1, x0), (1, x0, y1, y0)] {
                rim.clear();
                rim.extend(self.0.get(&(axis, at.to_bits())).into_iter().flatten().flat_map(|&(a, b, _)| [a, b]));
                rim.retain(|&t| (t - from) * (t - to) < 0.0);
                rim.push(to);
                rim.sort_unstable_by(|a, b| ((a - from).abs()).total_cmp(&(b - from).abs()));
                rim.dedup();
                let start = if axis == 0 { ground(from, at) } else { ground(at, from) };
                let mut last = prev.unwrap_or(start);
                for &t in &rim {
                    let p = if axis == 0 { ground(t, at) } else { ground(at, t) };
                    for v in [center, last, p] {
                        lane.extend(v.map(|c| c as f32));
                    }
                    last = p;
                }
                prev = Some(last);
            }
        }
        lines
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
