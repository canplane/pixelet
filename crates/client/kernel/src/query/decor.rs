// pxl-kernel/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the frame grows it (`grown_at`), off the ground's cells about the
//! cell, each read as asked (`World::surfaces`), and what objects place, each
//! struck by its model. A ray strikes it as the ground walk passes, only
//! where the frame draws it as held, at the leaf cells: a fold strikes none of
//! it. The walk follows the detail rule, not the chunk the view chose, which
//! may differ at a chunk's edge.

use pxl_spec::space::layer::lattice::{demand_reach, Cells};

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::DecorId;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::zoom::{cube, index_of, units, SHARD_ZOOM};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::ray::Ray;
use super::scatter::{grown_at, grown_of, Asked};
use super::surface::Surfaces;
use super::world::World;
use crate::decor::strike::{reach, strike};
use crate::layer::instance::{Instance, Meta};
use crate::layer::object::meta::placed;
use crate::layer::residency::Lines;
use crate::model::registry::Models;
use pxl_portable::area::{self, Shard};
use pxl_portable::window::Found;

/// How decor is struck: against its models, where the frame drawing at `tau` holds it.
#[derive(Clone, Copy)]
pub(super) struct Grown<'a> {
    pub(super) models: &'a Models,
    pub(super) tau: Tau,
}

/// One ray's decor strike, carried along its ground walk.
pub(super) struct Strike<'a> {
    models: &'a Models,
    /// How far any decor reaches from where it stands, and in leaf cells.
    pub(super) reach: f64,
    around: i64,
    /// How far along the ray decor is drawn as held.
    pub(super) held: f64,
    /// The cell met last, whose neighbors are struck already.
    last: Option<[i64; 2]>,
    pub(super) best: Option<(f64, Instance)>,
}

impl<'a> World<'a> {
    /// Whether the ground grows `i`, a candidate of `decor` standing on
    /// `face`, as the frame drawing its cells draws it (pxl-portable
    /// `grows`): its box void, the ground under it one it grows on, and no
    /// placed decor claiming its room.
    pub fn grows(&self, face: &Surfaces, decor: &Decor, i: &Instance) -> bool {
        grown_of(i).is_some_and(|g| pxl_portable::scatter::grows(&mut Asked::new(self, face), decor.row, face.cell(), &g))
    }

    /// What the ground stands on the leaf `cell`, each decor's, handed to
    /// `each` with the face it stands on until `each` says it has what it
    /// wants: the leaf's ground, held, read for every decor.
    /// Nothing where the windows do not hold the leaf's cells.
    fn grown_in(&self, cell: [i64; 2], mut each: impl FnMut(&mut Surfaces, &Decor, Instance) -> bool) -> bool {
        let node = (GROUND.cell_zoom(LEAF_ZOOM), cell);
        self.surfaces(node, GROUND.grid_zoom(LEAF_ZOOM), |face| {
            DECORS.iter().any(|decor| grown_at(&mut Asked::new(self, face), decor, cell).is_some_and(|i| each(face, decor, i)))
        })
        .exact()
        .unwrap_or(false)
    }

    /// A placed decor whose box shares room with `space`.
    pub fn placed_claim(&self, space: &Aabb) -> Option<Instance> {
        let mut met = None;
        self.each_placed_claim(space, |_, i| {
            met = Some(i);
            true
        });
        met
    }

    /// Each placed decor whose box shares room with `space`, with that box,
    /// until `each` says it has what it wants.
    fn each_placed_claim(&self, space: &Aabb, mut each: impl FnMut(Aabb, Instance) -> bool) {
        let r = reach();
        let around = Aabb {
            min: space.min.map(|v| v - r),
            max: space.max,
        };
        let mut done = false;
        self.objects.entries_in(&around, &mut |e| {
            if done {
                return;
            }
            if let Some((c, i)) = placed(e).and_then(|i| claim(&i).filter(|c| c.shares(*space)).map(|c| (c, i))) {
                done = each(c, i);
            }
        });
    }

    /// A decor the ground grows on the leaves whose box shares room with `space`.
    pub fn grown_claim(&self, space: &Aabb) -> Option<Instance> {
        let mut met = None;
        self.each_grown_over(space, |face, decor, i| {
            met = Some(i).filter(|i| self.grows(face, decor, i));
            met.is_some()
        });
        met
    }

    /// Every decor the ground's scatter stands on the leaves whose box shares
    /// room with `space`, whether or not its footing is there now: what an
    /// erase took away included.
    pub fn grown_over(&self, space: &Aabb) -> Vec<Instance> {
        let mut out = Vec::new();
        self.each_grown_over(space, |_, _, i| {
            out.push(i);
            false
        });
        out
    }

    /// Until `each` says it has what it wants.
    fn each_grown_over(&self, space: &Aabb, mut each: impl FnMut(&mut Surfaces, &Decor, Instance) -> bool) {
        let (r, unit) = (reach(), units(GROUND.cell()));
        let cells = |k: usize| libm::floor((space.min[k] - r) / unit) as i64..libm::ceil(space.max[k] / unit) as i64;
        for y in cells(1) {
            for x in cells(0) {
                if self.grown_in([x, y], |face, decor, i| claim(&i).is_some_and(|c| c.shares(*space)) && each(face, decor, i)) {
                    return;
                }
            }
        }
    }

    /// What the ground grows of `decor` standing in the leaf cell `at` lies
    /// in; none where it grows nothing there or the windows do not hold it.
    pub fn grown(&self, decor: &Decor, at: &Q3) -> Option<Instance> {
        let [x, y, z] = index_of(GROUND.cell(), at);
        let mut met = None;
        self.grown_in([x, y], |_, d, i| {
            met = Some(i).filter(|i| d.value() == decor.value() && i.grown_in().is_some_and(|c| c[2] == z));
            met.is_some()
        });
        met
    }

    /// The strike a ray carries, where decor is struck at all.
    pub(super) fn strike(&self, ray: &Ray) -> Option<Strike<'a>> {
        let Grown { models, tau } = self.decor?;
        // Held as it is out to where the leaves' tiles end.
        let held = match GROUND.z_at(tau, 0.0) {
            LEAF_ZOOM => (demand_reach(GROUND, LEAF_ZOOM) / tau.scale()).min(ray.reach),
            _ => return None,
        };
        let reach = reach();
        Some(Strike {
            models,
            reach,
            around: reach.ceil() as i64,
            held,
            last: None,
            best: None,
        })
    }

    /// Strikes the decor standing near the leaf cell `cell` the walk is in:
    /// those not near the cell met before it, since a walk only moves on.
    pub(super) fn strike_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        let (r, last) = (s.around, s.last.replace(cell));
        let near = |a: [i64; 2], b: [i64; 2]| (a[0] - b[0]).abs() <= r && (a[1] - b[1]).abs() <= r;
        // Placed decor stands where the ray's height over the held span, widened by the reach, can meet it.
        let (za, zb) = (ray.at(0.0)[2], ray.at(s.held)[2]);
        let (z0, z1) = (za.min(zb) - s.reach, za.max(zb) + s.reach);
        let fresh = |xy: [i64; 2]| !last.is_some_and(|l| near(xy, l));
        for y in cell[1] - r..=cell[1] + r {
            for x in cell[0] - r..=cell[0] + r {
                if fresh([x, y]) {
                    self.grown_near(s, ray, [x, y]);
                }
            }
        }
        // What is placed about it, the objects' tree walked once over every
        // cell: met in the cell its box's corner is in, once.
        let space = Aabb {
            min: [(cell[0] - r) as f64, (cell[1] - r) as f64, z0],
            max: [(cell[0] + r + 1) as f64, (cell[1] + r + 1) as f64, z1],
        };
        self.objects.entries_in(&space, &mut |e| {
            let c = e.cube().min;
            let own = [c[0].floor() as i64, c[1].floor() as i64];
            if let Some(i) = placed(e).filter(|_| space.holds(c) && fresh(own)) {
                meet(s, ray, i);
            }
        });
    }

    /// Whether decor `s` strikes may reach into the node `(zoom, c)`, X on
    /// the asker's lap, where no block under it fills: decor stands on the
    /// ground or where something is placed, and reaches no farther than its
    /// reach from there, so a node is out of it where, about it and as wide
    /// as the reach, the windows' relief holds no top over its bottom less the
    /// reach and the objects' pyramid no record, past the area none, as
    /// `shard`, what the descent established at the node, says. What is not
    /// held may.
    pub(super) fn may_strike(&self, s: &Strike<'a>, (zoom, c): Node<3>, shard: Shard) -> bool {
        // The nodes about it as wide as the reach, at the finest zoom that is.
        let mut z = zoom;
        while z > 0 && units(z) < s.reach {
            z -= 1;
        }
        let a = c.map(|v| v >> (zoom - z));
        let low = cube(zoom, c).min[2] - s.reach;
        let leaf = units(GROUND.cell());
        let node = (zoom, [c[0] as i32, c[1] as i32]);
        // The nine's lines share the walk down to where they part.
        let mut lines = Lines::default();
        let ground = (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let xy = (z, [a[0] + dx, a[1] + dy]);
                // Past the area no ground stands.
                if z >= SHARD_ZOOM && shard.past_leaf(node, area::tile_at(z, [xy.1[0] as i32, xy.1[1] as i32])) {
                    return false;
                }
                let mut place = Found::NONE;
                match self.terrain.relief_walked(&mut lines, &mut place, xy, GROUND.grid_zoom(z.min(LEAF_ZOOM))).value() {
                    Some([[_, hi], _]) => f64::from(hi) * leaf > low,
                    None => true,
                }
            })
        });
        // The 27 nodes about it at once, the objects' tree walked once.
        let side = units(z);
        let about = Aabb {
            min: [(a[0] - 1) as f64 * side, (a[1] - 1) as f64 * side, (a[2] - 1) as f64 * side],
            max: [(a[0] + 2) as f64 * side, (a[1] + 2) as f64 * side, (a[2] + 2) as f64 * side],
        };
        ground || self.objects.holds_in(&about, z)
    }

    fn grown_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        self.grown_in(cell, |face, decor, i| {
            if self.grows(face, decor, &i) {
                meet(s, ray, i);
            }
            false
        });
    }
}

/// The box a decor thing claims.
pub fn claim(i: &Instance) -> Option<Aabb> {
    let Meta::Decor(m) = i.meta;
    Some(Decor::of(DecorId::of(i.id)?).claim(i.at, &m))
}

fn meet(s: &mut Strike, ray: &Ray, i: Instance) {
    if let Some(t) = strike(s.models, &i, ray.origin, ray.direction, s.best.map_or(s.held, |b| b.0)) {
        s.best = Some((t, i));
    }
}
