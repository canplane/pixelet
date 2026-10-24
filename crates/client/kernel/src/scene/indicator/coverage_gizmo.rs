// pxl-kernel/scene/indicator/coverage_gizmo.rs

//! The coverage gizmo (the `world_gizmo` setting, beside the frame's), as
//! translucent debug lines over the ground: the eye's windows' squares
//! (layer/terrain/window.rs), each where the eye reads it at its zoom,
//! marked by how it stands, and the drawn ones' tiles and shards, each
//! colored as its `Mark` says (`gizmo_spec::coverage`).
//! Every line is cut at each edge's ends on it, and each stretch drawn once
//! in the highest mark over it, straight between its ends on the ground:
//! overlaid, the lines read as none of them, and laid along the ground they
//! wavered with every cell they crossed.

use fnv::{FnvHashMap, FnvHashSet};

use pxl_render::look::color::linear_rgb;
use pxl_spec::bake::tile::terrain::TerrainTile;
use pxl_spec::space::camera::eye::Observation;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::q3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::{base_of, units, within, Zoom, SHARD_ZOOM};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM, TOP_ZOOM};
use pxl_spec_core::window::{self, Slot, EYE_WINDOWS, FIRST_CELL, SLOTS, SQUARE_SHIFT};
use pxl_spec_core::{CHUNK_STEPS, LEAF_CELL};
use xpute_guest::global::Global;

use crate::kernel;
use crate::layer::terrain::window::{eye_slots, tile_of};
use crate::layer::Base as _;
use crate::query::World;
use crate::scene::indicator::gizmo_spec;
use crate::sched::edge::request_world_tick;
use crate::view::frame::pose::view;
use crate::view::lens::tau;
use crate::view::observation::{observation, ByEye};

/// What an edge marks, in the spec's order (`gizmo_spec::coverage`), which is
/// its rank: a stretch takes the mark of highest rank over it. Its look is the
/// spec's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mark {
    Drawn,
    DrawnFromFloor,
    Unbuilt,
    FloorOnly,
    NoTile,
    Tile,
    Shard,
}

const MARKS: usize = Mark::ALL.len();

impl Mark {
    const ALL: [Mark; 7] = [Mark::Drawn, Mark::DrawnFromFloor, Mark::Unbuilt, Mark::FloorOnly, Mark::NoTile, Mark::Tile, Mark::Shard];

    /// Its color, opacity and dash.
    fn look(self) -> (u32, f32, bool) {
        use gizmo_spec::coverage::*;
        match self {
            Mark::Drawn => (drawn::rgb, drawn::alpha, drawn::dashed),
            Mark::DrawnFromFloor => (drawn_from_floor::rgb, drawn_from_floor::alpha, drawn_from_floor::dashed),
            Mark::Unbuilt => (unbuilt::rgb, unbuilt::alpha, unbuilt::dashed),
            Mark::FloorOnly => (floor_only::rgb, floor_only::alpha, floor_only::dashed),
            Mark::NoTile => (no_tile::rgb, no_tile::alpha, no_tile::dashed),
            Mark::Tile => (tile::rgb, tile::alpha, tile::dashed),
            Mark::Shard => (shard::rgb, shard::alpha, shard::dashed),
        }
    }

    /// Its place in the spec's order.
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

    fn dashed(self) -> bool {
        self.look().2
    }
}

/// Dashes along a dashed stretch, every other one drawn.
const DASHES: u32 = 16;

/// A node of the ground's plane: its zoom and its column and row.
type Node2 = (Zoom, [i32; 2]);

/// What the gizmo outlines: the squares with their marks, then the drawn
/// ones' tiles and shards, each once.
#[derive(Default, PartialEq)]
struct Outline {
    squares: Vec<(Node2, Mark)>,
    tiles: Vec<Tile>,
    shards: Vec<Node2>,
}

#[derive(Default)]
struct Coverage {
    /// About where what is drawn is laid; none while hidden.
    drawn: Option<V3>,
    /// The lines then the faces, and where the faces begin.
    lane: (Vec<f32>, usize),
    /// Those laid this step, kept so their buffer is reused.
    next: (Vec<f32>, usize),
}

static COVERAGE: Global<Coverage> = Global::new();

/// Laid each step, and handed on only where its lines or where they are
/// laid about moved: a square landing moves its mark, and the ground under
/// it with it. Before the frame's list begins (scene/stage.rs), so its lines
/// and the origin they are drawn about are the same turn's.
pub fn coverage_gizmo_step(show: bool) {
    let c = COVERAGE.get(Coverage::default);
    let Some((scene, draw)) = kernel().parts() else { return };
    let (Some(o), Some(top)) = (observation().filter(|_| show), TerrainTile::top()) else {
        if c.drawn.take().is_some() {
            scene.coverage.set(draw, [0.0; 3], &[], 0);
            request_world_tick();
        }
        return;
    };
    // Its lines are f32 about the corner under the eye of a tile about as wide as
    // the eye is high: what is near resolves, and a move rebuilds them once a height.
    let height = (o.eye[2] - o.ground).max(1.0);
    let zoom = within(height).clamp(TOP_ZOOM as i32, LEAF_ZOOM as i32) as Zoom;
    let span = units(zoom);
    let eye = view().at().eye;
    let about = [(eye[0] / span).floor() * span, (eye[1] / span).floor() * span, 0.0];
    let outline = outline(&o, tau(), top);
    let mut lines = Lines::default();
    for &(n, mark) in &outline.squares {
        lines.piece(rect(n), mark);
    }
    for t in &outline.tiles {
        lines.square(t.rect(), Mark::Tile);
    }
    for &s in &outline.shards {
        lines.square(rect(s), Mark::Shard);
    }
    c.next.0.clear();
    c.next.1 = lines.lay(&mut c.next.0, about, &World::now());
    if c.drawn != Some(about) || c.next != c.lane {
        scene.coverage.set(draw, about, &c.next.0, c.next.1);
        c.drawn = Some(about);
        core::mem::swap(&mut c.next, &mut c.lane);
        request_world_tick();
    }
}

/// South-west corner and side, in units.
fn rect((zoom, [x, y]): Node2) -> (f64, f64, f64) {
    let base = base_of(zoom, [x, y, 0]);
    (q3::to_real(i64::from(base[0])), q3::to_real(i64::from(base[1])), units(zoom))
}

/// The goal, as 10-12's chunks cut the ground: each eye window's squares in
/// pieces a chunk's cells a side (`CHUNK_STEPS`), finest first, a piece
/// where the zoom its nearest point asks (`Tau::cell_at`, about the eye's
/// height, as a ray's descent asks) is its window's or finer, within the
/// reach, and not wholly under the finer window's pieces, which are read
/// there instead; none coarser than the source's top, which nothing asks
/// for. A piece is drawn where its square is present, or where a coarser
/// window's square is, which the cast reads in its place (pxl-spec-core
/// `window::step`): from the floor where that square's tile is the floor's
/// alone. One not drawn is marked by its line: no tile on it, only the
/// floor's, or a tile and its square not yet filled.
fn outline(o: &Observation, tau: Tau, top: Zoom) -> Outline {
    let held: [Option<[Slot; SLOTS as usize]>; EYE_WINDOWS as usize] = core::array::from_fn(|w| eye_slots(window::eye_cell(w as u32)));
    let present = |cell: Zoom, q: [i32; 2]| held[(cell - FIRST_CELL) as usize].is_some_and(|s| s[window::slot(q) as usize].holds(q));
    let over = (o.eye[2] - o.ground).max(0.0);
    let near = |(x0, y0, side): (f64, f64, f64)| {
        let x0 = world::nearest(x0 + side / 2.0, o.eye[0]) - side / 2.0;
        let gap = |lo: f64, e: f64| (lo - e).max(e - lo - side).max(0.0);
        let (gx, gy) = (gap(x0, o.eye[0]), gap(y0, o.eye[1]));
        gx * gx + gy * gy + over * over
    };
    let residency = TerrainTile::residency();
    // The floor's tiles are the top's no demand lines (layer/residency/floor.rs).
    let mut lined: FnvHashSet<Tile> = FnvHashSet::default();
    ByEye {
        tau,
        lattice: TerrainTile::LATTICE,
        top,
    }
    .requirements(o, &mut |p| {
        lined.insert(line_top(p, top));
    });
    let floor_only = |p: Tile| p.zoom <= top && !lined.contains(&p);
    let mut out = Outline::default();
    let mut read: FnvHashSet<Node2> = FnvHashSet::default();
    for cell in (FIRST_CELL..=LEAF_CELL).rev() {
        if GROUND.tile_zoom(cell) < top as i32 {
            continue;
        }
        let Some(slots) = held[(cell - FIRST_CELL) as usize] else { continue };
        let zoom = window::squares_zoom(cell);
        // Each square's pieces, a chunk's cells a side each, as 10-12's goal cut the ground.
        let k = SQUARE_SHIFT - CHUNK_STEPS;
        let pz = zoom + k;
        for slot in slots {
            let q = slot.square;
            let tile = tile_of(cell, q).0;
            // The square drawn in its place: its own, or the finest coarser one present.
            let drawn = (FIRST_CELL..=cell).rev().map(|c| (c, [q[0] >> (cell - c), q[1] >> (cell - c)])).find(|&(c, at)| present(c, at));
            for i in 0..1i32 << (2 * k) {
                let p = [(q[0] << k) + (i & ((1 << k) - 1)), (q[1] << k) + (i >> k)];
                let r2 = near(rect((pz, p)));
                if r2 > o.reach * o.reach || tau.cell_at(r2).clamp(FIRST_CELL, LEAF_CELL) < cell {
                    continue;
                }
                read.insert((pz, p));
                if (0..4).all(|c| read.contains(&(pz + 1, [2 * p[0] + (c & 1), 2 * p[1] + (c >> 1)]))) {
                    continue;
                }
                let mark = match drawn {
                    Some((c, at)) => {
                        out.tiles.push(tile);
                        let s = pz.saturating_sub(SHARD_ZOOM);
                        out.shards.push((pz.min(SHARD_ZOOM), [p[0] >> s, p[1] >> s]));
                        match c < cell && floor_only(tile_of(c, at).0) {
                            true => Mark::DrawnFromFloor,
                            false => Mark::Drawn,
                        }
                    }
                    None => match residency.line(tile).drawn {
                        None => Mark::NoTile,
                        Some(s) if floor_only(s) => Mark::FloorOnly,
                        Some(_) => Mark::Unbuilt,
                    },
                };
                out.squares.push(((pz, p), mark));
            }
        }
    }
    out.tiles.sort_unstable();
    out.tiles.dedup();
    out.shards.sort_unstable();
    out.shards.dedup();
    out
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
struct Lines(FnvHashMap<(u8, u64), Vec<Stretch>>, Vec<(f64, f64, f64)>);

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
        let mut grounds: FnvHashMap<(u64, u64), V3> = FnvHashMap::default();
        let mut ends: Vec<(f64, bool, Mark)> = Vec::new();
        // Each point asked once, by the lines and the faces alike.
        let mut ground = |x: f64, y: f64| -> V3 {
            *grounds.entry((x.to_bits(), y.to_bits())).or_insert_with(|| {
                let z = access.surface_support(&world::point(x, y, 0.0)).value().flatten().unwrap_or(0.0);
                [world::nearest(x, origin[0]) - origin[0], y - origin[1], z - origin[2]]
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
        let mut coarser = FnvHashSet::default();
        for &(x0, y0, side) in &self.1 {
            let mut s = side * 2.0;
            while s <= widest {
                coarser.insert(key(((x0 / s).floor() * s, (y0 / s).floor() * s, s)));
                s *= 2.0;
            }
        }
        let mut laid = FnvHashSet::default();
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
