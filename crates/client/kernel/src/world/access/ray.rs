// pxl-kernel/world/access/ray.rs

//! The first thing a ray meets: ground walked a region at a time, decks and placed objects, at the detail the distance or a zoom asks.

use pxl_spec::bake::constant::OBJECT_FINEST;
use pxl_spec::bake::format::drawn;
use pxl_spec::cell::object::{object_value_is_air, object_value_is_voxel};
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::{Aabb, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::{EXTENT, GROUND};
use pxl_spec::space::layer::tile::{Tile, LEAF_ZOOM};

use crate::layer::object::store::EntryAt;
use pxl_spec::space::coord::zoom::{base_of, cube, units, Zoom};

use super::{answer::*, column::*, ground::*};

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
pub(super) struct Region {
    pub(super) base: [f64; 2],
    pub(super) side: f64,
    pub(super) tile: Tile,
    /// Index bounds and whether read from the tile asked; none if nothing is resident.
    pub(super) terrain: Option<((f64, f64), bool)>,
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
        let mut t = 0.0;
        while t < ray.reach {
            let (zoom, cells, until) = detail.at(t);
            let end = until.min(ray.reach);
            let objects = self.objects(ray, (t, end), cells);
            let limit = objects.map_or(end, |o| o.t);
            let ground = self.ground_first(ray, (t, limit), zoom, &mut lack, &mut answered).filter(|g| g.t <= limit);
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

    pub(super) fn ground_first(&self, ray: &Ray, (t0, t1): (f64, f64), zoom: Zoom, lack: &mut Option<Tile>, answered: &mut bool) -> Option<Hit> {
        let mut t = t0;
        while t < t1 {
            self.visit();
            if let Some(out) = self.clear(ray, t, zoom, t1) {
                *answered = true;
                t = out;
                continue;
            }
            let r = self.region(&ray.at(t + NUDGE), zoom);
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
    pub(super) fn clear(&self, ray: &Ray, t: f64, zoom: Zoom, until: f64) -> Option<f64> {
        let from = ray.at(t + NUDGE);
        let at = on_ground(&from);
        (GROUND.top()..zoom).find_map(|coarser| {
            let body = self.terrain.get(GROUND.cell_of(coarser, &at).tile())?;
            let e = EXTENT.cell_of(coarser, &at);
            let [_, hi] = drawn(body.index.extent()[e.index() as usize]);
            let out = exit(ray, corner(e.zoom(), &from), units(e.zoom())).min(until).max(t + NUDGE);
            let (za, zb) = (ray.at(t)[2], ray.at(out)[2]);
            (za.min(zb) > hi as f64 * units(GROUND.cell())).then_some(out)
        })
    }

    pub(super) fn region(&self, p: &V3, zoom: Zoom) -> Region {
        let at = on_ground(p);
        let tile = GROUND.cell_of(zoom, &at).tile();
        let (cell_zoom, terrain) = match self.terrain.drawn(tile) {
            Some((src, body)) => {
                // The drawn-from tile's index cell, or the asked tile if smaller: one bound
                // and one tile across it.
                let e = EXTENT.cell_of(src.zoom, &at);
                let [lo, hi] = drawn(body.index.extent()[e.index() as usize]);
                let exact = self.terrain.source(tile).is_some();
                (e.zoom().max(zoom), Some(((lo as f64 * units(GROUND.cell()), hi as f64 * units(GROUND.cell())), exact)))
            }
            None => (zoom, None),
        };
        Region {
            base: corner(cell_zoom, p),
            side: units(cell_zoom),
            tile,
            terrain,
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
            let hit = if zoom == LEAF_ZOOM {
                self.column(ray, cell, from, to)
            } else {
                self.folded(ray, cell, from, to, zoom)
            };
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

    pub(super) fn with_decks(&self, ray: &Ray, boxes: &[[f64; 2]], (corner, side): ([f64; 2], f64), (t0, t1): (f64, f64), ground: Option<Hit>) -> Option<Hit> {
        let mut best = ground;
        for &[lo, hi] in boxes {
            let cube = Aabb {
                min: [corner[0], corner[1], lo],
                max: [corner[0] + side, corner[1] + side, hi],
            };
            let Some((t, normal)) = ray_box(ray, cube) else { continue };
            if t < t0 || t > t1 || best.is_some_and(|b| b.t <= t) {
                continue;
            }
            best = Some(hit(ray, t, normal, cube, Struck::Ground));
        }
        best
    }

    pub(super) fn column(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64) -> Option<Hit> {
        let side = units(GROUND.cell());
        let c = self.composed([ix, iy]).value()?;
        let ground = self.ground_column(ray, &c, t0, t1);
        self.with_decks(ray, &c.lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    /// Ground hit in a leaf column: a cell under the top not taken by a voxel
    /// entry and not in bore air. Steps by cell only once under the top.
    pub(super) fn ground_column(&self, ray: &Ray, c: &Composed, t0: f64, t1: f64) -> Option<Hit> {
        let ([ix, iy], top) = (c.cell, c.top?);
        let side = units(GROUND.cell());
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
            if (cz as f64) * side < top && from > 0.0 && self.holds(c, cz) {
                // Through the column's side on the entry cell, else through floor or ceiling.
                let normal = if from == t0 {
                    wall_normal(ray, from, side)
                } else {
                    [0.0, 0.0, if dz > 0.0 { -1.0 } else { 1.0 }]
                };
                let min = [ix as f64 * side, iy as f64 * side, cz as f64 * side];
                return Some(hit(ray, from, normal, Aabb { min, max: min.map(|v| v + side) }, Struck::Ground));
            }
            if leave >= t1 {
                return None;
            }
            cz += if dz > 0.0 { 1 } else { -1 };
            from = leave;
        }
    }

    pub(super) fn folded(&self, ray: &Ray, [ix, iy]: [i64; 2], t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let cell = GROUND.cell_of(zoom, &base_of(cell_zoom, [ix, iy, 0]));
        let g = self.ground(cell).value()?;
        let ground = g.top().and_then(|top| self.folded_ground(ray, [ix, iy], top, t0, t1, zoom));
        let lines = self.lines_over(cell, g.height);
        self.with_decks(ray, &lines.boxes, ([ix as f64 * side, iy as f64 * side], side), (t0, t1), ground)
    }

    pub(super) fn folded_ground(&self, ray: &Ray, [ix, iy]: [i64; 2], top: f64, t0: f64, t1: f64, zoom: Zoom) -> Option<Hit> {
        let cell_zoom = GROUND.cell_zoom(zoom);
        let side = units(cell_zoom);
        let (za, zb) = (ray.at(t0)[2], ray.at(t1)[2]);
        let (t, normal) = if za <= top {
            (t0 > 0.0).then(|| (t0, wall_normal(ray, t0, side)))?
        } else if zb <= top {
            ((top - ray.origin[2]) / ray.direction[2], [0.0, 0.0, 1.0])
        } else {
            return None;
        };
        let cz = ((ray.at(t)[2] - normal[2] * NUDGE) / side).floor();
        let min = [ix as f64 * side, iy as f64 * side, cz * side];
        Some(hit(ray, t, normal, Aabb { min, max: min.map(|v| v + side) }, Struck::Ground))
    }

    /// First object the ray meets, nearest first (`Store::along_ray`), in cells
    /// of `cell_zoom`: a leaf no finer than the cell is hit at its own cube, a
    /// cell holding finer is hit whole by what it folds to, as it is drawn.
    pub(super) fn objects(&self, ray: &Ray, (t0, t1): (f64, f64), cell_zoom: Zoom) -> Option<Hit> {
        let mut met = None;
        self.objects.along_ray(ray.origin, ray.direction, (t0, t1), cell_zoom, &mut |cell| {
            let (e, folded) = self.objects.node(cell_zoom, cell);
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
}
