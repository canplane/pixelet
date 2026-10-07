// pxl-kernel/world/query/ray.rs

//! The first thing a ray meets: ground walked a region at a time, decks and placed objects, at the detail the distance or a zoom asks.

use crate::layer::residency::Lines;
use crate::layer::terrain::Drawn;
use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_voxel};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::{EXTENT, GROUND};
use pxl_spec::space::layer::tile::Tile;

use crate::layer::object::store::EntryAt;
use pxl_spec::space::coord::world::node_near;
use pxl_spec::space::coord::zoom::{cube, units, Zoom, CUBE_ZOOM, SHARD_ZOOM};
use pxl_spec::space::hierarchy::morton::child_cube;
use pxl_spec::space::hierarchy::tree::Node;

use super::{column::*, ground::*};
use crate::layer::Query;
use crate::layer::*;

/// Probe distance past a boundary, in units: far below the lattice's finest
/// step and far above f64 error at world coordinates.
pub(super) const NUDGE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Detail {
    Zoom(Zoom),
    /// The zoom the LOD rule wants at each distance from the origin, so a
    /// ray's walk takes about as many steps per zoom whatever the zoom.
    Distance(Tau),
}

impl Detail {
    /// The ground's tile zoom from `t`, the cell zoom objects are met in, and
    /// how far both hold. At a stated zoom objects are met as they are held.
    pub(super) fn at(self, t: f64) -> (Zoom, Zoom, f64) {
        match self {
            Detail::Zoom(z) => (z, OBJECT_FINEST, f64::INFINITY),
            Detail::Distance(tau) => {
                let zoom = GROUND.z_at(tau, t);
                let until = if zoom == GROUND.top() { f64::INFINITY } else { GROUND.demand_reach(zoom) / tau.scale() };
                (zoom, tau.cell_at(t).min(OBJECT_FINEST), until)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: V3,
    pub direction: V3,
    pub reach: f64,
}

impl Ray {
    pub fn at(&self, t: f64) -> V3 {
        std::array::from_fn(|k| self.origin[k] + self.direction[k] * t)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Struck {
    Ground,
    Object(EntryAt),
    /// Objects folded into a cell above the leaves; no single object is met.
    Objects,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub t: f64,
    pub point: V3,
    pub normal: V3,
    /// The box struck.
    pub cube: Aabb,
    pub struck: Struck,
}

/// A square across which neither the tile nor the index cell changes.
pub(super) struct Region<'a> {
    pub(super) base: [f64; 2],
    pub(super) side: f64,
    pub(super) tile: Tile,
    /// Index bounds and whether read from the tile asked; none if nothing is resident.
    pub(super) terrain: Option<((f64, f64), bool)>,
    /// What the tile is drawn from, which every cell walked in it reads.
    pub(super) drawn: Option<Drawn<'a>>,
}

pub(super) fn exit(ray: &Ray, base: [f64; 2], side: f64) -> f64 {
    (0..2)
        .map(|k| {
            let d = ray.direction[k];
            if d > 1e-12 {
                (base[k] + side - ray.origin[k]) / d
            } else if d < -1e-12 {
                (base[k] - ray.origin[k]) / d
            } else {
                f64::INFINITY
            }
        })
        .fold(f64::INFINITY, f64::min)
}

pub(super) fn wall_normal(ray: &Ray, t: f64, side: f64) -> V3 {
    let p = ray.at(t);
    let off = |v: f64| {
        let r = v.rem_euclid(side);
        r.min(side - r)
    };
    let k = if off(p[0]) <= off(p[1]) { 0 } else { 1 };
    let mut n = [0.0; 3];
    n[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    n
}

pub(super) fn ray_box(ray: &Ray, cube: Aabb) -> Option<(f64, V3)> {
    let (enter, _, k) = cube.span_along(ray.origin, ray.direction)?;
    let mut normal = [0.0; 3];
    normal[k] = if ray.direction[k] > 0.0 { -1.0 } else { 1.0 };
    (enter >= 0.0 && enter <= ray.reach).then_some((enter, normal))
}

/// Where the ray meets the whole cell; if already inside at `from`, there,
/// through the nearest side. None if it starts inside.
pub(super) fn cell_hit(ray: &Ray, cell: [i64; 3], cell_zoom: Zoom, from: f64) -> Option<Hit> {
    let cube = cube(cell_zoom, cell);
    let (t, normal) = match ray_box(ray, cube) {
        Some((t, normal)) if t >= from => (t, normal),
        _ => (cube.holds(ray.at(from)) && from > 0.0).then(|| (from, wall_normal(ray, from, units(cell_zoom))))?,
    };
    Some(hit(ray, t, normal, cube, Struck::Objects))
}

pub(super) fn hit(ray: &Ray, t: f64, normal: V3, cube: Aabb, struck: Struck) -> Hit {
    Hit {
        t,
        point: ray.at(t),
        normal,
        cube,
        struck,
    }
}

/// A point's place on the ground lattice, its height dropped.
pub(super) fn on_ground(p: &V3) -> Pvf3 {
    world::point(p[0], p[1], 0.0)
}

/// The corner of `cell_zoom`'s cell under `p`, in the frame `p` is in: a ray
/// across the antimeridian walks on, where the cell's stored X wraps.
pub(super) fn corner(cell_zoom: Zoom, p: &V3) -> [f64; 2] {
    let side = units(cell_zoom);
    [(p[0] / side).floor() * side, (p[1] / side).floor() * side]
}

impl<'a> Access<'a> {
    /// The first thing `ray` meets within its reach. Objects (resident whole)
    /// are found first; ground is walked only up to them, so an object is never
    /// exact over ground not known to lie behind it. What the ray starts inside
    /// is not met, so an eye under the ground is not stopped where it stands.
    ///
    /// Ground is walked region by region: skipped where the ray stays above the
    /// index bound, walked cell by cell otherwise. A coarse or missing region
    /// the ray may enter is the lack. Resident coarser tiles are tried first
    /// (`clear`) so far-off ground is skipped in one test per zoom.
    pub fn first_hit(&self, ray: &Ray, detail: Detail) -> Answer<Option<Hit>> {
        let mut lack: Option<Tile> = None;
        let mut answered = false;
        let mut lines = Lines::default();
        let mut t = 0.0;
        while t < ray.reach {
            let (zoom, cells, until) = detail.at(t);
            let end = until.min(ray.reach);
            let objects = self.objects(ray, (t, end), cells);
            let limit = objects.map_or(end, |o| o.t);
            let ground = self.ground_first(ray, (t, limit), zoom, &mut lines, &mut lack, &mut answered).filter(|g| g.t <= limit);
            if let Some(hit) = ground.or(objects) {
                return match lack {
                    Some(l) => Answer::Coarser(Some(hit), l),
                    None => Answer::Exact(Some(hit)),
                };
            }
            t = end;
        }
        match (lack, answered) {
            (None, _) => Answer::Exact(None),
            (Some(l), true) => Answer::Coarser(None, l),
            (Some(l), false) => Answer::Nothing(l),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn ground_first(&self, ray: &Ray, (t0, t1): (f64, f64), zoom: Zoom, lines: &mut Lines, lack: &mut Option<Tile>, answered: &mut bool) -> Option<Hit> {
        let mut t = t0;
        while t < t1 {
            self.visit();
            if let Some(out) = self.clear(ray, t, zoom, t1, lines) {
                *answered = true;
                t = out;
                continue;
            }
            let r = self.region(&ray.at(t + NUDGE), zoom, lines);
            let out = exit(ray, r.base, r.side).min(t1).max(t + NUDGE).min(ray.reach);
            let low = ray.at(t)[2].min(ray.at(out)[2]);
            let under = r.terrain.is_some_and(|((_, hi), _)| low <= hi);
            match r.terrain {
                Some((_, exact)) => {
                    *answered = true;
                    if under && !exact {
                        lack.get_or_insert(r.tile);
                    }
                }
                None => {
                    lack.get_or_insert(r.tile);
                }
            }
            if under {
                if let Some(hit) = self.walk(ray, &r, t, out, zoom) {
                    return Some(hit);
                }
            }
            t = out;
        }
        None
    }

    /// Far edge of the largest resident index cell coarser than `z` the ray
    /// passes over entirely, or none. Never past `until`, where another zoom applies.
    pub(super) fn clear(&self, ray: &Ray, t: f64, zoom: Zoom, until: f64, lines: &mut Lines) -> Option<f64> {
        let from = ray.at(t + NUDGE);
        let at = on_ground(&from);
        (GROUND.top()..zoom).find_map(|coarser| {
            let (e, cell) = (EXTENT.cell_of(coarser, &at), GROUND.grid_zoom(coarser));
            let line = self.terrain.seek(lines, e.tile());
            // Only ground known exactly lets a ray pass over it.
            let hi = match self.terrain.bound(&line, e.node(), cell) {
                Answer::Exact(b) => b.map_or(f64::NEG_INFINITY, |b| b.aabb.max[2]),
                _ => return None,
            };
            let out = exit(ray, corner(e.zoom(), &from), units(e.zoom())).min(until).max(t + NUDGE);
            let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
            (za.min(zb) > hi).then_some(out)
        })
    }

    pub(super) fn region(&self, p: &V3, zoom: Zoom, lines: &mut Lines) -> Region<'a> {
        // One index cell of the tile at `zoom`: one bound across it, the whole tile where none is known.
        let (e, cell) = (EXTENT.cell_of(zoom, &on_ground(p)), GROUND.grid_zoom(zoom));
        let line = self.terrain.seek(lines, e.tile());
        let (cell_zoom, terrain) = match self.terrain.bound(&line, e.node(), cell) {
            Answer::Nothing(_) => (zoom, None),
            known => {
                let exact = matches!(known, Answer::Exact(_));
                let held = known.value().flatten().map_or((f64::INFINITY, f64::NEG_INFINITY), |b| (b.aabb.min[2], b.aabb.max[2]));
                (e.zoom(), Some((held, exact)))
            }
        };
        Region {
            base: corner(cell_zoom, p),
            side: units(cell_zoom),
            tile: e.tile(),
            terrain,
            drawn: self.terrain.drawn_of(&line),
        }
    }

    /// Walks cell columns at `z` inside `r` (Amanatides–Woo). Stops on the
    /// region's cells, not on `t1`: the two distances round differently, and a
    /// column past the edge would be read as this region's.
    pub(super) fn walk(&self, ray: &Ray, r: &Region, t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let side = units(GROUND.cell_zoom(zoom));
        let across = (r.side / side) as i64;
        let first = r.base.map(|v| (v / side).floor() as i64);
        let inside = |c: [i64; 2]| (0..2).all(|k| (first[k]..first[k] + across).contains(&c[k]));
        let [dx, dy, _] = ray.direction;
        let start = ray.at(t0 + NUDGE);
        let mut cell = [(start[0] / side).floor() as i64, (start[1] / side).floor() as i64];
        let axis = |d: f64, c: i64, o: f64| -> (f64, f64) {
            if d.abs() < 1e-12 {
                (f64::INFINITY, f64::INFINITY)
            } else {
                (side / d.abs(), ((c + i64::from(d > 0.0)) as f64 * side - o) / d)
            }
        };
        let (delta_x, mut next_x) = axis(dx, cell[0], ray.origin[0]);
        let (delta_y, mut next_y) = axis(dy, cell[1], ray.origin[1]);
        let mut from = t0;
        while from < t1 && inside(cell) {
            self.visit();
            let to = next_x.min(next_y).min(t1);
            let hit = r.drawn.as_ref().and_then(|d| self.column(ray, cell, zoom, (from, to), d));
            if hit.is_some() {
                return hit;
            }
            if next_x < next_y {
                cell[0] += if dx > 0.0 { 1 } else { -1 };
                next_x += delta_x;
            } else {
                cell[1] += if dy > 0.0 { 1 } else { -1 };
                next_y += delta_y;
            }
            from = to;
        }
        None
    }

    pub(super) fn column(&self, ray: &Ray, cell: [i64; 2], zoom: Zoom, (t0, t1): (f64, f64), d: &Drawn) -> Option<Hit> {
        let c = self.composed_in(cell, zoom, d).value()?;
        self.ground_column(ray, &c, t0, t1)
    }

    /// Ground hit in a column: the first leaf-tall cell the ground holds
    /// (`holds`). Steps by cell only once under the column's ceiling.
    pub(super) fn ground_column(&self, ray: &Ray, c: &Composed, t0: f64, t1: f64) -> Option<Hit> {
        let (side, across) = (units(GROUND.cell()), units(GROUND.cell_zoom(c.zoom)));
        let ([ix, iy], top) = (c.cell, c.ceiling()? as f64 * side);
        let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
        let dz = ray.direction[2];
        let at_z = |z: f64| (z - ray.origin[2]) / dz;
        let mut from = t0;
        if za >= top {
            if zb >= top {
                return None;
            }
            from = at_z(top);
        }
        let mut cz = (ray.at(from)[2] / side).floor() as i64;
        if from <= 0.0 && self.holds(c, cz) {
            return None;
        }
        loop {
            let leave = if dz.abs() < 1e-12 {
                t1
            } else {
                let face = if dz > 0.0 { (cz + 1) as f64 * side } else { cz as f64 * side };
                at_z(face).min(t1)
            };
            if from > 0.0 && self.holds(c, cz) {
                // Through the column's side on the entry cell, else through floor or ceiling.
                let normal = if from == t0 {
                    wall_normal(ray, from, across)
                } else {
                    [0.0, 0.0, if dz > 0.0 { -1.0 } else { 1.0 }]
                };
                let min = [ix as f64 * across, iy as f64 * across, cz as f64 * side];
                let max = [min[0] + across, min[1] + across, min[2] + side];
                return Some(hit(ray, from, normal, Aabb { min, max }, Struck::Ground));
            }
            if leave >= t1 {
                return None;
            }
            cz += if dz > 0.0 { 1 } else { -1 };
            from = leave;
        }
    }

    /// First object the ray meets, in cells of `cell_zoom`, each read through the objects' query.
    pub(super) fn objects(&self, ray: &Ray, (t0, t1): (f64, f64), cell_zoom: Zoom) -> Option<Hit> {
        let mut met = None;
        self.objects_along((ray.origin, ray.direction), (t0, t1), cell_zoom, &mut |cell| {
            let Answer::Exact(cells) = self.objects.read((cell_zoom, cell), cell_zoom) else { return false };
            let (e, folded) = cells.node();
            met = (object_value_is_voxel(e.state) && !object_value_is_air(e.state))
                .then(|| match folded {
                    true => cell_hit(ray, cell, cell_zoom, t0),
                    false => ray_box(ray, e.cube()).map(|(t, normal)| hit(ray, t, normal, e.cube(), Struck::Object(e.id()))),
                })
                .flatten();
            met.is_some()
        });
        met
    }

    /// Every cell of `cell_zoom` that may hold objects along the ray, in ray
    /// order, until `each` stops it: a descent over the objects' bounds, each
    /// node read whole, from the column roots the ray crosses, nearest child first.
    fn objects_along(&self, ray: (V3, V3), span: (f64, f64), cell_zoom: Zoom, each: &mut dyn FnMut([i64; 3]) -> bool) {
        let side = units(cell_zoom);
        cells_along(ray, span, units(SHARD_ZOOM), &mut |[x, y, half]| {
            let mut met = [(0.0, [0; 3]); 8];
            let mut n = 0;
            for s in 0..8 {
                let at = child_cube(SHARD_ZOOM, [x, y, 0], s);
                if at[2] != half || !self.objects_held(CUBE_ZOOM, at) {
                    continue;
                }
                if let Some(c) = slab(ray, &cube(CUBE_ZOOM, at).snapped_out(side), span) {
                    met[n] = (c.0, at);
                    n += 1;
                }
            }
            met[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
            met[..n].iter().any(|&(_, at)| self.objects_down(ray, span, cell_zoom, (CUBE_ZOOM, at), each))
        });
    }

    fn objects_down(&self, ray: (V3, V3), span: (f64, f64), cell_zoom: Zoom, (zoom, at): Node<3>, each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
        let side = units(cell_zoom);
        let here = node_near(zoom, at, ray.0[0]);
        let Some(s) = slab(ray, &cube(zoom, here).snapped_out(side), span) else { return false };
        if zoom == cell_zoom {
            return each(here);
        }
        let mut met = [(0.0, [0; 3]); 8];
        let mut n = 0;
        for k in 0..8 {
            let at = child_cube(zoom, at, k);
            if !self.objects_held(zoom + 1, at) {
                continue;
            }
            if let Some(cs) = slab(ray, &cube(zoom + 1, node_near(zoom + 1, at, ray.0[0])).snapped_out(side), s) {
                met[n] = (cs.0, at);
                n += 1;
            }
        }
        met[..n].sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        met[..n].iter().any(|&(_, at)| self.objects_down(ray, s, cell_zoom, (zoom + 1, at), each))
    }

    /// Whether anything is placed in the cube, read whole: one find.
    fn objects_held(&self, zoom: Zoom, at: [i64; 3]) -> bool {
        self.objects.bounds((zoom, at), zoom).value().flatten().is_some()
    }
}

fn slab((o, d): (V3, V3), b: &Aabb, (t0, t1): (f64, f64)) -> Option<(f64, f64)> {
    let (enter, leave, _) = b.span_along(o, d)?;
    let (enter, leave) = (enter.max(t0), leave.min(t1));
    (enter <= leave).then_some((enter, leave))
}

/// Amanatides–Woo traversal of `side` cells; returns whether `each` stopped it.
fn cells_along((o, d): (V3, V3), (t0, t1): (f64, f64), side: f64, each: &mut dyn FnMut([i64; 3]) -> bool) -> bool {
    let start = [0, 1, 2].map(|k| o[k] + d[k] * t0);
    let mut cell = start.map(|v| (v / side).floor() as i64);
    let mut next = [0.0; 3];
    let mut delta = [0.0; 3];
    for k in 0..3 {
        if d[k].abs() < 1e-12 {
            (next[k], delta[k]) = (f64::INFINITY, f64::INFINITY);
        } else {
            let face = (cell[k] + i64::from(d[k] > 0.0)) as f64 * side;
            (next[k], delta[k]) = ((face - o[k]) / d[k], side / d[k].abs());
        }
    }
    loop {
        let k = (0..3).min_by(|&a, &b| next[a].total_cmp(&next[b])).unwrap_or(0);
        if each(cell) {
            return true;
        }
        if next[k] >= t1 {
            return false;
        }
        cell[k] += if d[k] > 0.0 { 1 } else { -1 };
        next[k] += delta[k];
    }
}
