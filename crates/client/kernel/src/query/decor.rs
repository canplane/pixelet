// pxl-kernel/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the chunks grow it (`grown_at`), on one read of the ground about
//! the cell (`World::surfaces`), and what objects place, each
//! struck by its model. A ray strikes it as the ground walk passes, only
//! where the chunks draw it as held, at the leaf cells: a fold strikes none of
//! it. The walk follows the detail rule, not the chunk the view chose, which
//! may differ at a chunk's edge.

use pxl_spec::space::layer::lattice::{demand_reach, Cells};
use std::collections::TryReserveError;

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::Block;
use pxl_spec::block::DecorId;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world::wrap_cell;
use pxl_spec::space::coord::zoom::{base_of, cube, index_of, units};
use pxl_spec::space::hierarchy::tree::Node;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::ray::Ray;
use super::scatter::{grown_at, ring};
use super::surface::Surfaces;
use super::world::World;
use crate::decor::strike::{reach, strike};
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::meta::placed;
use crate::layer::Query;
use crate::model::registry::Models;

/// How decor is struck: against its models, where the chunks drawing at `tau` hold it.
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
    /// What placed decor claims about the candidate struck last.
    placed: Vec<Aabb>,
}

impl<'a> World<'a> {
    /// Whether the ground grows `i`, a candidate of `decor`, on `face`, as
    /// the chunks drawing its cells draw it: every block of its box void,
    /// and every one under its base ground `decor` grows on, each asked of
    /// the world's one query at the face's cells; and no placed decor claims
    /// its room, told from
    /// `placed`, the boxes placed decor claims about it (`placed_claims`).
    /// Air placed in it or under it is no void.
    pub fn grows(&self, face: &mut Surfaces, decor: &Decor, i: &Instance, placed: &[Aabb]) -> bool {
        let (Source::Ground([x, y, z]), Meta::Decor(m)) = (i.source, i.meta) else { return false };
        let [w, d, h] = decor.variants[m.variant as usize].cells(m.turn).map(i64::from);
        let unit = units(GROUND.cell());
        let (side, cell) = (face.side(), face.cell());
        let block = |v: f64| (v / side).floor() as i64;
        (0..d).all(|j| {
            (0..w).all(|k| {
                let base = base_of(GROUND.cell(), [x + k, y + j, z]);
                let [bx, by, floor] = [i64::from(base[0]), i64::from(base[1]), i64::from(base[2])].map(q3::to_real).map(|v| v + unit / 2.0);
                // The base's cell up and down: the box's blocks void, and
                // the one under it ground the decor grows on.
                let [cx, cy] = [bx, by].map(block);
                let at = |z: f64| self.block((cell, [cx, cy, block(z)])).value().flatten();
                matches!(at(floor - unit), Some(Block::Ground(g, _)) if decor.allows(g)) && (0..h).all(|n| at(floor + n as f64 * unit).is_none())
            })
        }) && claim(i).is_some_and(|c| !placed.iter().any(|p| p.shares(c)))
    }

    /// `grows` for one candidate, what placed decor claims read about it
    /// alone into `placed`; not where the heap refused it room.
    fn grows_alone(&self, face: &mut Surfaces, decor: &Decor, i: &Instance, placed: &mut Vec<Aabb>) -> bool {
        claim(i).is_some_and(|c| self.placed_claims(&c, placed).is_ok()) && self.grows(face, decor, i, placed)
    }

    /// What the ground stands on the leaf `cell`, each decor's, handed to
    /// `each` with the face it stands on until `each` says it has what it
    /// wants: the leaf's ground read once about the cell for every decor.
    /// Nothing where the leaf is not resident.
    fn grown_in(&self, cell: [i64; 2], mut each: impl FnMut(&mut Surfaces, &Decor, Instance) -> bool) -> bool {
        let node = (GROUND.cell_zoom(LEAF_ZOOM), cell);
        self.surfaces(node, GROUND.grid_zoom(LEAF_ZOOM), ring(&DECORS), |face| {
            DECORS.iter().any(|decor| grown_at(face, decor, cell).is_some_and(|i| each(face, decor, i)))
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

    /// The boxes placed decor claims that share room with `space`, into
    /// `out`: what every candidate whose box lies in `space` is told from
    /// (`grows`), the store walked once for all of them.
    pub fn placed_claims(&self, space: &Aabb, out: &mut Vec<Aabb>) -> Result<(), TryReserveError> {
        out.clear();
        let mut grown = Ok(());
        self.each_placed_claim(space, |c, _| {
            grown = out.try_reserve(1).map(|()| out.push(c));
            grown.is_err()
        });
        grown
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
        let (mut met, mut placed) = (None, Vec::new());
        self.each_grown_over(space, |face, decor, i| {
            met = Some(i).filter(|i| self.grows_alone(face, decor, i, &mut placed));
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
    /// in; none where it grows nothing there or its leaf tile is not resident.
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
            placed: Vec::new(),
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
        for y in cell[1] - r..=cell[1] + r {
            for x in cell[0] - r..=cell[0] + r {
                if last.is_some_and(|l| near([x, y], l)) {
                    continue;
                }
                self.grown_near(s, ray, [x, y]);
                let space = Aabb {
                    min: [x as f64, y as f64, z0],
                    max: [(x + 1) as f64, (y + 1) as f64, z1],
                };
                // Met in the cell its box's corner is in, once.
                self.objects.entries_in(&space, &mut |e| {
                    let c = e.cube().min;
                    let own = (0..2).all(|k| (space.min[k]..space.max[k]).contains(&c[k]));
                    if let Some(i) = placed(e).filter(|_| own && space.holds(c)) {
                        meet(s, ray, i);
                    }
                });
            }
        }
    }

    /// Whether decor `s` strikes may reach into the node `(zoom, c)`, X on
    /// the asker's lap, where no block under it fills: decor stands on the
    /// ground or where something is placed, and reaches no farther than its
    /// reach from there, so a node is out of it where, about it and as wide
    /// as the reach, the ground's index holds no top over its bottom less the
    /// reach and the objects' pyramid no record. What is not held may.
    pub(super) fn may_strike(&self, s: &Strike<'a>, (zoom, c): Node<3>) -> bool {
        // The nodes about it as wide as the reach, at the finest zoom that is.
        let mut z = zoom;
        while z > 0 && units(z) < s.reach {
            z -= 1;
        }
        let a = c.map(|v| v >> (zoom - z));
        let low = cube(zoom, c).min[2] - s.reach;
        let leaf = units(GROUND.cell());
        let ground = (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let xy = (z, [a[0] + dx, a[1] + dy]);
                match self.terrain.relief(&self.terrain.cursor(xy, GROUND.grid_zoom(z.min(LEAF_ZOOM))), xy).value() {
                    Some(r) => r.is_some_and(|[_, hi]| f64::from(hi) * leaf > low),
                    None => true,
                }
            })
        });
        ground
            || (-1..=1).any(|dz| {
                (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let n = wrap_cell(z, [a[0] + dx, a[1] + dy, a[2] + dz]);
                        self.objects.cursor((z, n), z).holds()
                    })
                })
            })
    }

    fn grown_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        let mut placed = core::mem::take(&mut s.placed);
        self.grown_in(cell, |face, decor, i| {
            if self.grows_alone(face, decor, &i, &mut placed) {
                meet(s, ray, i);
            }
            false
        });
        s.placed = placed;
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
