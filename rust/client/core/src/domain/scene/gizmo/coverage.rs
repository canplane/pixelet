// pxl-core/domain/scene/gizmo/coverage.rs

//! The coverage gizmo, shown with the frame gizmo under the `world_gizmo`
//! setting and drawn as translucent debug lines over the ground. It outlines
//! the bricks of the eye's clip levels (domain/layer/terrain/clipmap.rs)
//! where the eye reads each at its zoom, each marked by its state, and the
//! tiles and shards of the drawn bricks. Each outline is colored as its
//! `Mark` says (`gizmo_spec::coverage`). Every line is cut at the ends of
//! every edge on it, and each stretch between cuts is drawn once, in the
//! highest mark over it, straight between its ends on the ground.

use fnv::{FnvHashMap, FnvHashSet};
use pxl_spec::brick;

use pxl_engine::clipmap::FIRST_CELL_ZOOM;
use pxl_render::gpu::draw::DrawList;
use pxl_render::look::color::linear_rgb;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::bake::tile::Tile as _;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::cell_zoom_at;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::zoom::{
    base_of, units, within, Zoom, LEAF_CELL_ZOOM, LEAF_ZOOM, MIN_ZOOM,
};
use pxl_spec::space::coord::{q3, world};
use pxl_spec::space::layer::area;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::global::Global;

use crate::domain::layer::terrain::clipmap::{eye_bricks, Drawn, EyeBrick};
use crate::domain::layer::Base as _;
use crate::domain::query::World;
use crate::domain::scene::gizmo::debug_line::DebugMesh;
use crate::domain::scene::gizmo::gizmo_spec;
use crate::domain::scene::model::draw_list;
use crate::domain::view::frame::pose::view;
use crate::domain::view::observation::{observation, ByEye};
use crate::sys::sched::edge::request_world_tick;

/// What an edge marks. The variants follow the spec's order
/// (`gizmo_spec::coverage`), and that order is their rank: a stretch takes
/// the highest-ranked mark over it. The spec also gives each mark's look.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mark {
    Drawn,
    DrawnFromFloor,
    Unbuilt,
    FloorOnly,
    NoTile,
    Quadrant,
    Shard,
}

const MARKS: usize = Mark::ALL.len();

impl Mark {
    const ALL: [Mark; 7] = [
        Mark::Drawn,
        Mark::DrawnFromFloor,
        Mark::Unbuilt,
        Mark::FloorOnly,
        Mark::NoTile,
        Mark::Quadrant,
        Mark::Shard,
    ];

    /// The mark's color, its opacity, and whether it is dashed.
    fn look(self) -> (u32, f32, bool) {
        use gizmo_spec::coverage::*;
        match self {
            Mark::Drawn => (drawn::rgb, drawn::alpha, drawn::dashed),
            Mark::DrawnFromFloor => (
                drawn_from_floor::rgb,
                drawn_from_floor::alpha,
                drawn_from_floor::dashed,
            ),
            Mark::Unbuilt => (unbuilt::rgb, unbuilt::alpha, unbuilt::dashed),
            Mark::FloorOnly => (floor_only::rgb, floor_only::alpha, floor_only::dashed),
            Mark::NoTile => (no_tile::rgb, no_tile::alpha, no_tile::dashed),
            Mark::Quadrant => (tile::rgb, tile::alpha, tile::dashed),
            Mark::Shard => (shard::rgb, shard::alpha, shard::dashed),
        }
    }

    fn rank(self) -> usize {
        self as usize
    }

    fn of_rank(rank: usize) -> Mark {
        Mark::ALL[rank]
    }

    fn rgba(self) -> [f32; 4] {
        let (rgb, a, _) = self.look();
        let [r, g, b] = linear_rgb(rgb).map(|c| c as f32);
        [r, g, b, a]
    }

    fn is_dashed(self) -> bool {
        self.look().2
    }
}

/// The pieces a dashed stretch is cut into. Every other piece is drawn.
const DASHES: u32 = 16;

/// A node of the ground's plane: its zoom and its column and row.
type Node2 = (Zoom, [i32; 2]);

/// What the gizmo outlines: the bricks with their marks, and the tiles and
/// shards of the drawn bricks, each listed once.
#[derive(Default, PartialEq)]
struct Outline {
    bricks: Vec<(Node2, Mark)>,
    tiles: Vec<Quadrant>,
    shards: Vec<Node2>,
}

#[derive(Default)]
struct Coverage {
    /// The origin the drawn lines are laid about, or None while the gizmo is
    /// hidden.
    drawn: Option<V3>,
    /// The line vertices followed by the face vertices, and the index where
    /// the faces begin.
    vertices: (Vec<f32>, usize),
    /// The vertices laid this step. They are kept between steps so their
    /// buffer is reused.
    next: (Vec<f32>, usize),
    mesh: DebugMesh,
}

static COVERAGE: Global<Coverage> = Global::new();

pub fn coverage_gizmo_draw(draw: &mut DrawList) {
    COVERAGE.get(Coverage::default).mesh.draw(draw);
}

/// Lays the gizmo every step, but hands it to the renderer only when its
/// lines or their origin changed. A brick that lands changes its mark, and
/// the ground under it changes with it. It must run before the frame's list
/// begins (domain/scene/stage.rs), so the lines and the origin they are drawn
/// about come from the same turn.
pub fn coverage_gizmo_step(show: bool) {
    let c = COVERAGE.get(Coverage::default);
    let draw = draw_list();
    let (Some(o), Some(top)) = (observation().filter(|_| show), TerrainTile::min_zoom()) else {
        if c.drawn.take().is_some() {
            c.mesh.set(draw, [0.0; 3], &[], 0);
            request_world_tick();
        }
        return;
    };
    // The lines are f32 values about an origin: the corner, under the eye, of
    // a tile about as wide as the eye is high. What is near the eye keeps its
    // precision, and the eye must move about its own height before the lines
    // are rebuilt about a new origin.
    let height = (o.eye[2] - o.ground).max(1.0);
    let zoom = within(height).clamp(MIN_ZOOM as i32, LEAF_ZOOM as i32) as Zoom;
    let span = units(zoom);
    let eye = view().at().eye;
    let about = [
        (eye[0] / span).floor() * span,
        (eye[1] / span).floor() * span,
        0.0,
    ];
    let outline = outline(&o, top);
    let mut lines = LineMap::default();
    for &(n, mark) in &outline.bricks {
        lines.piece(rect(n), mark);
    }
    for t in &outline.tiles {
        lines.brick(t.rect(), Mark::Quadrant);
    }
    for &s in &outline.shards {
        lines.brick(rect(s), Mark::Shard);
    }
    c.next.0.clear();
    c.next.1 = lines.lay(&mut c.next.0, about, &World::now());
    if c.drawn != Some(about) || c.next != c.vertices {
        c.mesh.set(draw, about, &c.next.0, c.next.1);
        c.drawn = Some(about);
        core::mem::swap(&mut c.next, &mut c.vertices);
        request_world_tick();
    }
}

/// The south-west corner and the side of a node's square, in units.
fn rect((zoom, [x, y]): Node2) -> (f64, f64, f64) {
    let base = base_of(zoom, [x, y, 0]);
    (
        q3::to_real(i64::from(base[0])),
        q3::to_real(i64::from(base[1])),
        units(zoom),
    )
}

/// The outline of what the eye should read, cut over the ground the way the
/// observation cuts it (domain/view/observation.rs `bricks`). It walks each
/// eye clip level's bricks, finest first. A brick is outlined, as a piece,
/// when it lies within the reach, when the zoom its nearest point asks is its
/// clip level's or finer (pxl-spec space/camera/grid.rs `cell_zoom_at`,
/// measured with the eye's height as a descending ray measures it), and when
/// the finer clip level's bricks, which are read there instead, do not cover
/// it whole. No brick coarser than the source's top is outlined, since
/// nothing asks for one. A piece is drawn where its brick is present, or
/// where a coarser clip level's brick is present, which the cast then reads
/// in its place (pxl-engine `clipmap::step`). Such a piece is drawn from the
/// floor when that coarser brick's tile is held by the floor alone. A piece
/// not drawn is marked with the reason: no tile on it, only the floor's tile,
/// or a tile whose brick is not filled yet.
fn outline(o: &Observation, top: Zoom) -> Outline {
    let height = (o.eye[2] - o.ground).max(0.0);
    let near = |(x0, y0, side): (f64, f64, f64)| {
        let x0 = world::nearest(x0 + side / 2.0, o.eye[0]) - side / 2.0;
        let gap = |lo: f64, e: f64| (lo - e).max(e - lo - side).max(0.0);
        let (gx, gy) = (gap(x0, o.eye[0]), gap(y0, o.eye[1]));
        gx * gx + gy * gy + height * height
    };
    // A tile at the top or coarser that no demand line passes through is held
    // by the floor alone (domain/layer/floor.rs). `lined` collects
    // the top tile of every demand line.
    let mut lined: FnvHashSet<Quadrant> = FnvHashSet::default();
    ByEye {
        lattice: TerrainTile::LATTICE,
        top,
    }
    .requirements(o, &mut |p| {
        lined.insert(line_top(p, top));
    });
    let floor_only = |p: Quadrant| p.zoom <= top && !lined.contains(&p);
    let mut out = Outline::default();
    let mut read: FnvHashSet<Node2> = FnvHashSet::default();
    for cell_zoom in (FIRST_CELL_ZOOM..=LEAF_CELL_ZOOM).rev() {
        if GROUND.tile_zoom(cell_zoom) < top as i32 {
            continue;
        }
        let Some(bricks) = eye_bricks(cell_zoom) else {
            continue;
        };
        let pz = brick::bricks_zoom(cell_zoom);
        for EyeBrick { at: p, tile, drawn } in bricks {
            let r2 = near(rect((pz, p)));
            if r2 > o.demand_dist * o.demand_dist
                || cell_zoom_at(r2 as f32).clamp(FIRST_CELL_ZOOM, LEAF_CELL_ZOOM) < cell_zoom
            {
                continue;
            }
            read.insert((pz, p));
            if (0..4).all(|c| read.contains(&(pz + 1, [2 * p[0] + (c & 1), 2 * p[1] + (c >> 1)]))) {
                continue;
            }
            let mark = match drawn {
                Drawn::Cells {
                    cell_zoom: c,
                    tile: from,
                } => {
                    out.tiles.push(tile);
                    out.shards.push(area::shard_over(pz, p));
                    match c < cell_zoom && floor_only(from) {
                        true => Mark::DrawnFromFloor,
                        false => Mark::Drawn,
                    }
                }
                Drawn::Nothing { finest: None } => Mark::NoTile,
                Drawn::Nothing { finest: Some(s) } if floor_only(s) => Mark::FloorOnly,
                Drawn::Nothing { finest: Some(_) } => Mark::Unbuilt,
            };
            out.bricks.push(((pz, p), mark));
        }
    }
    out.tiles.sort_unstable();
    out.tiles.dedup();
    out.shards.sort_unstable();
    out.shards.dedup();
    out
}

fn line_top(mut p: Quadrant, top: Zoom) -> Quadrant {
    while p.zoom > top {
        p = p.parent();
    }
    p
}

/// The edges, grouped by the line they lie on. A line is keyed by its axis
/// (0 along x, 1 along y) and by where it crosses the other axis, and it
/// holds each edge on it as a stretch with its mark. The second field holds
/// the pieces' bricks, whose faces hide the parts of the gizmo behind them.
#[derive(Default)]
struct LineMap(FnvHashMap<(u8, u64), Vec<Stretch>>, Vec<(f64, f64, f64)>);

/// Where an edge starts and ends along its line, and what it marks.
type Stretch = (f64, f64, Mark);

impl LineMap {
    /// Adds the four edges of a square given as `rect` gives it: its
    /// south-west corner and its side.
    fn brick(&mut self, (x0, y0, side): (f64, f64, f64), mark: Mark) {
        let (x1, y1) = (x0 + side, y0 + side);
        for (axis, at, from, to) in [
            (0, y0, x0, x1),
            (0, y1, x0, x1),
            (1, x0, y0, y1),
            (1, x1, y0, y1),
        ] {
            self.0
                .entry((axis, at.to_bits()))
                .or_default()
                .push((from, to, mark));
        }
    }

    /// Adds a piece's brick: its edges, and its face among the faces.
    fn piece(&mut self, rect: (f64, f64, f64), mark: Mark) {
        self.brick(rect, mark);
        self.1.push(rect);
    }

    /// Lays the vertices about `origin`. Each line is swept once and cut at
    /// the end of every edge on it. Each stretch between cuts takes the
    /// highest mark over it and is drawn straight between its ends on the
    /// ground. Then each piece's face is laid as a fan from its center through
    /// every cut on its edges, so its rim follows the lines exactly. Returns
    /// the index where the faces begin.
    fn lay(&self, vertices: &mut Vec<f32>, origin: V3, access: &World) -> usize {
        let mut grounds: FnvHashMap<(u64, u64), V3> = FnvHashMap::default();
        let mut ends: Vec<(f64, bool, Mark)> = Vec::new();
        // The ground under each point is asked once, and the lines and the
        // faces share the answer.
        let mut ground = |x: f64, y: f64| -> V3 {
            *grounds
                .entry((x.to_bits(), y.to_bits()))
                .or_insert_with(|| {
                    let z = access
                        .surface_support(&world::point(x, y, 0.0))
                        .val()
                        .flatten()
                        .unwrap_or(0.0);
                    [
                        world::nearest(x, origin[0]) - origin[0],
                        y - origin[1],
                        z - origin[2],
                    ]
                })
        };
        for (&(axis, at), edges) in &self.0 {
            let at = f64::from_bits(at);
            let mut point = |t: f64| {
                if axis == 0 {
                    ground(t, at)
                } else {
                    ground(at, t)
                }
            };
            ends.clear();
            ends.extend(
                edges
                    .iter()
                    .flat_map(|&(a, b, m)| [(a, true, m), (b, false, m)]),
            );
            ends.sort_unstable_by(|x, y| x.0.total_cmp(&y.0));
            let mut over = [0u32; MARKS];
            let mut from: Option<f64> = None;
            let mut k = 0;
            while k < ends.len() {
                let t = ends[k].0;
                // Every edge on the line cuts it, so where a coarse edge lies
                // over finer ones it is drawn in their stretches, following
                // the ground at the same points they do.
                if let (Some(f), Some(m)) = (from, (0..MARKS).rev().find(|&m| over[m] > 0)) {
                    stretch(vertices, point(f), point(t), Mark::of_rank(m));
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
        let lines = vertices.len();
        let mut rim: Vec<f64> = Vec::new();
        // A face is laid only for a piece with no finer piece inside it. A
        // coarse face would hide the finer edges wherever the ground dips
        // under it.
        let key = |(x, y, side): (f64, f64, f64)| (x.to_bits(), y.to_bits(), side.to_bits());
        let widest = self.1.iter().map(|r| r.2).fold(0.0, f64::max);
        let mut coarser = FnvHashSet::default();
        for &(x0, y0, side) in &self.1 {
            let mut s = side * 2.0;
            while s <= widest {
                coarser.insert(key(((x0 / s).floor() * s, (y0 / s).floor() * s, s)));
                s *= 2.0;
            }
        }
        let mut laid = FnvHashSet::default();
        for &(x0, y0, side) in self
            .1
            .iter()
            .filter(|&&r| !coarser.contains(&key(r)) && laid.insert(key(r)))
        {
            let (x1, y1) = (x0 + side, y0 + side);
            let center = ground(x0 + side / 2.0, y0 + side / 2.0);
            let mut prev: Option<V3> = None;
            // The rim runs counterclockwise from the south-west corner,
            // through each edge's cuts and then its far corner.
            for (axis, at, from, to) in [
                (0, y0, x0, x1),
                (1, x1, y0, y1),
                (0, y1, x1, x0),
                (1, x0, y1, y0),
            ] {
                rim.clear();
                rim.extend(
                    self.0
                        .get(&(axis, at.to_bits()))
                        .into_iter()
                        .flatten()
                        .flat_map(|&(a, b, _)| [a, b]),
                );
                rim.retain(|&t| (t - from) * (t - to) < 0.0);
                rim.push(to);
                rim.sort_unstable_by(|a, b| ((a - from).abs()).total_cmp(&(b - from).abs()));
                rim.dedup();
                let start = if axis == 0 {
                    ground(from, at)
                } else {
                    ground(at, from)
                };
                let mut last = prev.unwrap_or(start);
                for &t in &rim {
                    let p = if axis == 0 {
                        ground(t, at)
                    } else {
                        ground(at, t)
                    };
                    for v in [center, last, p] {
                        vertices.extend(v.map(|c| c as f32));
                    }
                    last = p;
                }
                prev = Some(last);
            }
        }
        lines
    }
}

fn stretch(vertices: &mut Vec<f32>, a: V3, b: V3, mark: Mark) {
    let pieces = if mark.is_dashed() { DASHES } else { 1 };
    let at = |t: u32| -> [f32; 3] {
        core::array::from_fn(|i| (a[i] + (b[i] - a[i]) * t as f64 / pieces as f64) as f32)
    };
    for i in (0..pieces).step_by(2) {
        for p in [at(i), at(i + 1)] {
            vertices.extend_from_slice(&p);
            vertices.extend_from_slice(&mark.rgba());
        }
    }
}
