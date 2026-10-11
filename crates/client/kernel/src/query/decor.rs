// pxl-kernel/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the chunks grow it (`grown_at`), and what objects place, each
//! struck by its model. A ray strikes it as the ground walk passes, only
//! where the chunks draw it as held, at the leaf cells: a fold strikes none of
//! it. The walk follows the detail rule, not the chunk the view chose, which
//! may differ at a chunk's edge.

use pxl_spec::block::decor::{Decor, DECORS};
use pxl_spec::block::stand::Block;
use pxl_spec::block::DecorId;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::{base_of, index_of, units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::ray::{Detail, Ray};
use super::scatter::grown_at;
use super::support::block_in;
use super::world::World;
use crate::decor::strike::{reach, strike};
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::meta::placed;
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
    /// The column met last, whose neighbors are struck already.
    last: Option<[i64; 2]>,
    pub(super) best: Option<(f64, Instance)>,
}

impl<'a> World<'a> {
    /// Whether the ground grows `i`, a candidate of `decor`, as the tile zoom
    /// `zoom`'s chunks draw it: every block of its box void, and every one
    /// under its base ground `decor` grows on. Air placed in it or under it is
    /// no void.
    pub fn grows(&self, decor: &Decor, i: &Instance, zoom: Zoom) -> bool {
        let (Source::Ground([x, y, z]), Meta::Decor(m)) = (i.source, i.meta) else { return false };
        let [w, d, h] = decor.variants[m.variant as usize].cells(m.turn).map(i64::from);
        let (unit, half) = (units(GROUND.cell()), units(GROUND.cell()) / 2.0);
        (0..d).all(|j| {
            (0..w).all(|k| {
                let base = base_of(GROUND.cell(), [x + k, y + j, z]);
                let center = pvf3::gather(pvf3::X(&base) + pvf3::from_real(half), pvf3::Y(&base) + pvf3::from_real(half), pvf3::Z(&base));
                let floor = pvf3::to_real(pvf3::Z(&base)) + half;
                // The box's blocks and the one under it, from one composition of their column.
                let footing = self.runs_at(&center, zoom, |runs, _| {
                    matches!(block_in(runs, floor - unit), Some(Block::Ground(g)) if decor.allows(g)) && (0..h).all(|n| block_in(runs, floor + n as f64 * unit).is_none())
                });
                footing.value() == Some(true)
            })
        }) && claim(i).is_some_and(|c| self.placed_claim(&c).is_none())
    }

    /// A placed decor whose box shares room with `space`.
    pub fn placed_claim(&self, space: &Aabb) -> Option<Instance> {
        let r = reach();
        let around = Aabb {
            min: space.min.map(|v| v - r),
            max: space.max,
        };
        let mut met = None;
        self.objects.entries_in(&around, &mut |e| {
            if met.is_none() {
                met = placed(e).filter(|i| claim(i).is_some_and(|c| shares(&c, space)));
            }
        });
        met
    }

    /// A decor the ground grows on the leaves whose box shares room with `space`.
    pub fn grown_claim(&self, space: &Aabb) -> Option<Instance> {
        let mut met = None;
        self.each_grown_over(space, |decor, i| {
            met = Some(i).filter(|i| self.grows(decor, i, LEAF_ZOOM));
            met.is_some()
        });
        met
    }

    /// Every decor the ground's scatter stands on the leaves whose box shares
    /// room with `space`, whether or not its footing is there now: what an
    /// erase took away included.
    pub fn grown_over(&self, space: &Aabb) -> Vec<Instance> {
        let mut out = Vec::new();
        self.each_grown_over(space, |_, i| {
            out.push(i);
            false
        });
        out
    }

    /// Until `each` says it has what it wants.
    fn each_grown_over(&self, space: &Aabb, mut each: impl FnMut(&Decor, Instance) -> bool) {
        let (r, unit) = (reach(), units(GROUND.cell()));
        let cells = |k: usize| libm::floor((space.min[k] - r) / unit) as i64..libm::ceil(space.max[k] / unit) as i64;
        for y in cells(1) {
            for x in cells(0) {
                for decor in &DECORS {
                    let grown = grown_at(self, decor, [x, y]).filter(|i| claim(i).is_some_and(|c| shares(&c, space)));
                    if grown.is_some_and(|i| each(decor, i)) {
                        return;
                    }
                }
            }
        }
    }

    /// What the ground grows of `decor` standing in the leaf cell `at` lies
    /// in; none where it grows nothing there or its leaf tile is not resident.
    pub fn grown(&self, decor: &Decor, at: &Pvf3) -> Option<Instance> {
        let [x, y, z] = index_of(GROUND.cell(), at);
        grown_at(self, decor, [x, y]).filter(|i| i.grown_in().is_some_and(|c| c[2] == z))
    }

    /// The strike a ray carries, where decor is struck at all.
    pub(super) fn strike(&self, ray: &Ray) -> Option<Strike<'a>> {
        let Grown { models, tau } = self.decor?;
        let held = match Detail::Distance(tau).at(0.0) {
            (LEAF_ZOOM, _, until) => until.min(ray.reach),
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

    /// Strikes the decor standing near the leaf column `cell` the walk is in:
    /// those not near the column met before it, since a walk only moves on.
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
                let column = Aabb {
                    min: [x as f64, y as f64, z0],
                    max: [(x + 1) as f64, (y + 1) as f64, z1],
                };
                // Met in the column its box's corner is in, once.
                self.objects.entries_in(&column, &mut |e| {
                    let c = e.cube().min;
                    let own = (0..2).all(|k| (column.min[k]..column.max[k]).contains(&c[k]));
                    if let Some(i) = placed(e).filter(|_| own && column.holds(c)) {
                        meet(s, ray, i);
                    }
                });
            }
        }
    }

    fn grown_near(&self, s: &mut Strike<'a>, ray: &Ray, cell: [i64; 2]) {
        for decor in &DECORS {
            let Some(i) = grown_at(self, decor, cell) else { continue };
            if self.grows(decor, &i, LEAF_ZOOM) {
                meet(s, ray, i);
            }
        }
    }
}

/// The box a decor thing claims.
pub fn claim(i: &Instance) -> Option<Aabb> {
    let Meta::Decor(m) = i.meta;
    Some(Decor::of(DecorId::of(i.id)?).claim(i.at, &m))
}

/// Whether two boxes share room, not only a face.
pub fn shares(a: &Aabb, b: &Aabb) -> bool {
    (0..3).all(|k| a.min[k] < b.max[k] && b.min[k] < a.max[k])
}

fn meet(s: &mut Strike, ray: &Ray, i: Instance) {
    if let Some(t) = strike(s.models, &i, ray.origin, ray.direction, s.best.map_or(s.held, |b| b.0)) {
        s.best = Some((t, i));
    }
}
