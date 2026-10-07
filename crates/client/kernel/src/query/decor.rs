// pxl-kernel/query/decor.rs

//! Decor as the world answers it: what the ground grows, evaluated cell by
//! cell as the chunks grow it (`grown_at`), and what objects place, each
//! struck by its model. It is met only where it is drawn as held, at the
//! leaf cells: a fold strikes none of it.

use pxl_spec::cell::decor::{DecorKind, KINDS};
use pxl_spec::cell::object::{decor_id_from_num, object_value_is_decor};
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::zoom::{base_of, index_of};
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::tile::LEAF_ZOOM;

use super::ray::{Detail, Ray};
use super::world::{covers, Over, World};
use crate::decor::strike::{reach, strike};
use crate::layer::instance::{Instance, Meta, Source};
use crate::layer::object::meta::user_decor_pick;
use crate::layer::terrain::scatter::grown_at;
use crate::layer::Query;
use pxl_spec::cell::decor::DecorMeta;
use pxl_spec::space::camera::grid::Tau;

/// A ray's decor is gathered over squares this many leaf cells a side along
/// it, each widened by the decor's reach.
const STRIDE: f64 = 8.0;

impl World<'_> {
    /// What the ground grows of `kind` standing in the leaf cell `at` lies
    /// in; none where it grows nothing there or its leaf tile is not resident.
    pub fn grown(&self, kind: &DecorKind, at: &Pvf3) -> Option<Instance> {
        let [x, y, z] = index_of(GROUND.cell(), at);
        self.grown_in(kind, [x, y]).filter(|i| matches!(i.source, Source::Ground([_, _, e]) if e == z))
    }

    fn grown_in(&self, kind: &DecorKind, [x, y]: [i64; 2]) -> Option<Instance> {
        let leaf = GROUND.cell_of(LEAF_ZOOM, &base_of(GROUND.cell(), [x, y, 0]));
        let (zoom, [tx, ty]) = leaf.tile().node();
        let source = self.terrain.read((zoom, [tx, ty, 0]), GROUND.grid_zoom(zoom)).exact()?.view();
        grown_at(kind, &source, [x, y], leaf.in_tile())
    }

    /// The decor `ray` strikes first, nearer than `limit`, and where along it.
    pub(super) fn decor_first(&self, ray: &Ray, limit: f64) -> Option<(f64, Instance)> {
        let models = self.models?;
        // Held at leaf cells only: as far as the view draws them so at its finest.
        let held = held_until(Detail::Distance(Tau::FINEST), limit);
        if held <= 0.0 {
            return None;
        }
        let reach = reach(models);
        let (mut cells, mut placed) = (Vec::new(), Vec::new());
        let mut t = 0.0;
        while t < held {
            let end = (t + STRIDE).min(held);
            let (a, b) = (ray.at(t), ray.at(end));
            let around = Aabb {
                min: std::array::from_fn(|k| a[k].min(b[k]) - reach),
                max: std::array::from_fn(|k| a[k].max(b[k]) + reach),
            };
            for y in around.min[1].floor() as i64..=around.max[1].floor() as i64 {
                for x in around.min[0].floor() as i64..=around.max[0].floor() as i64 {
                    cells.push([x, y]);
                }
            }
            self.objects.entries_in(&around, &mut |e| {
                if object_value_is_decor(e.state) {
                    placed.push(*e);
                }
            });
            t = end;
        }
        cells.sort_unstable();
        cells.dedup();
        placed.sort_unstable_by_key(|e| (e.zoom, e.at));
        placed.dedup_by_key(|e| (e.zoom, e.at));
        let mut best: Option<(f64, Instance)> = None;
        let mut meet = |i: Instance| {
            if let Some(t) = strike(models, &i, ray.origin, ray.direction, best.map_or(held, |b| b.0)) {
                best = Some((t, i));
            }
        };
        for &cell in &cells {
            for kind in &KINDS {
                let Some(i) = self.grown_in(kind, cell) else { continue };
                let Source::Ground(at) = i.source else { continue };
                let held = self.objects.read((GROUND.cell(), at), GROUND.cell()).value().is_some_and(|c| c.holds());
                if !covers(Over::Leaf { held }) {
                    meet(i);
                }
            }
        }
        for e in &placed {
            // It stands at its cube's bottom middle, turned and varied by where it is.
            let c = e.cube();
            let kind = DecorKind::of(decor_id_from_num(e.state));
            let (variant, rotation) = user_decor_pick(e.corner().i, kind.variant_count());
            meet(Instance {
                id: kind.value(),
                at: [(c.min[0] + c.max[0]) / 2.0, (c.min[1] + c.max[1]) / 2.0, c.min[2]],
                source: Source::Object(e.id()),
                meta: Meta::Decor(DecorMeta { variant, rotation, scale: 1.0 }),
            });
        }
        best
    }
}

/// How far along a ray, up to `limit`, its cells are drawn held, at the leaf
/// cells: the cell a distance draws grows with it, so the edge is found by halving.
fn held_until(detail: Detail, limit: f64) -> f64 {
    let held = |t: f64| detail.at(t).1 >= GROUND.cell();
    if held(limit) {
        return limit;
    }
    let (mut lo, mut hi) = (0.0, limit);
    if !held(lo) {
        return 0.0;
    }
    for _ in 0..48 {
        let mid = (lo + hi) / 2.0;
        if held(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}
