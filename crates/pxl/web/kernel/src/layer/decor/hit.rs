// pxl-kernel/layer/decor/hit.rs

//! Which decor the pointer's ray strikes, against the drawn triangles of the
//! z16 chunks' decor: decor paints no voxel, so the access's ray passes through
//! it. A placed instance resolves to its entry; a scattered one to its cell.

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::{cross3, dot3, sub3, Aabb, V3};
use xpute_runtime::mem::base::ptr_at;

use crate::kernel::kernel;
use crate::layer::decor::asset_store::kind_assets;
use crate::layer::decor::instance::{decor_groups, GroupLanes, NO_ENTRY};
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::EntryAt;
use crate::world::chunk::table::Scheduler;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecorStruck {
    Placed(EntryAt),
    Scattered(Scattered),
}

/// The cell it stands in, which a leaf written there overrides, and how it
/// is drawn, for a preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scattered {
    pub cell: [i64; 3],
    pub group: u32,
    pub at: [f64; 3],
    pub rotation: f64,
    pub scale: f64,
}

/// The ray is in the root frame, relative to `map_origin`. A decor wins only
/// strictly nearer than `max_dist`, the world's hit, as a depth test decides.
pub fn raycast_decor(origin: V3, direction: V3, map_origin: IVec, max_dist: f64) -> Option<DecorStruck> {
    let (_, sched) = kernel().world()?;
    let frame = [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin))];
    let hit = hit_decor(sched, frame, origin, direction, max_dist)?;
    if hit.band == NO_ENTRY {
        return Some(DecorStruck::Scattered(Scattered {
            cell: hit.at.map(|v| (v / CELL.side()).floor() as i64),
            group: hit.group,
            at: hit.at,
            rotation: hit.rotation,
            scale: hit.scale,
        }));
    }
    // The lane holds the band's i32 as a word.
    Some(DecorStruck::Placed(EntryAt {
        col_idx: {
            let (x0, y0, _) = hit.cell.rect();
            L2_IDX(ivec::gather(x0 as i64, y0 as i64, 0))
        },
        band_idx: hit.band as i32,
        pos: hit.pos,
    }))
}

/// `band` is NO_ENTRY for a scattered instance.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DecorHit {
    pub cell: crate::world::pyramid::chunk::Chunk,
    pub band: u32,
    pub pos: u32,
    pub at: [f64; 3],
    pub group: u32,
    pub rotation: f64,
    pub scale: f64,
    pub t: f64,
}

/// None while it loads.
fn group_models() -> impl Iterator<Item = Option<u32>> {
    kind_assets().iter().flat_map(|e| e.models.iter().copied())
}

/// The largest model's bounding radius at the largest decor size; only for
/// skipping cells the ray never comes near.
fn decor_reach(sched: &Scheduler) -> f64 {
    let radius = group_models()
        .flatten()
        .filter_map(|m| sched.models.mesh(m))
        .map(|m| m.radius as f64 * m.scale as f64)
        .fold(0.0, f64::max);
    radius * DECOR_SIZE
}

/// Hidden instances are tested too: the hidden one is what the eraser is over.
fn hit_decor(sched: &Scheduler, origin: [f64; 2], o: V3, d: [f64; 3], max_dist: f64) -> Option<DecorHit> {
    let mut best: Option<DecorHit> = None;
    let mut limit = max_dist;
    let reach = decor_reach(sched);
    for (cell, base_x, base_y, words) in crate::layer::decor::cell::shown() {
        // Only z16 cells name their placed entries; edits are made close by.
        if cell.level() != 0 {
            continue;
        }
        let side = cell.rect().2;
        let tx = base_x - origin[0];
        let ty = base_y - origin[1];
        if !ray_meets_square(&o, &d, tx - reach, ty - reach, tx + side + reach, ty + side + reach, limit) {
            continue;
        }
        for (group, ((offset, count), model)) in decor_groups(words).zip(group_models()).enumerate() {
            let Some(mesh) = model.and_then(|m| sched.models.mesh(m)) else { continue };
            let lanes = GroupLanes {
                first: offset as usize,
                count: count as usize,
            };
            let lane = |k: usize, i: usize| f32::from_bits(words[lanes.f32_at(k, i)]) as f64;
            // SAFETY: the model's block, positions first, indices past the vertex words.
            let (positions, indices) = unsafe {
                (
                    core::slice::from_raw_parts(ptr_at::<f32>(mesh.at), mesh.vertex_cnt as usize * 3),
                    core::slice::from_raw_parts(
                        (ptr_at::<u32>(mesh.at)).add(mesh.vertex_cnt as usize * pxl_render::frame::draw::MODEL_VERTEX_WORDS as usize),
                        mesh.index_cnt as usize,
                    ),
                )
            };
            for i in 0..lanes.count {
                let band = words[lanes.band_at(i)];
                let c = [tx + lane(0, i), ty + lane(1, i), lane(2, i)];
                let spin = lane(3, i);
                let s = lane(4, i) * mesh.scale as f64;
                let r = mesh.radius as f64 * s;
                let p = sub3(&o, &c);
                if !ray_meets_sphere(&p, &d, r, limit) {
                    continue;
                }
                // Into the model's frame: spin undone, the stand-up quarter turn about X, scale.
                let (sn, cs) = libm::sincos(-spin);
                let unturn = |v: [f64; 3]| [v[0] * cs - v[1] * sn, v[0] * sn + v[1] * cs, v[2]];
                let to_model = |v: [f64; 3]| {
                    let u = unturn(v);
                    [u[0] / s, u[2] / s, -u[1] / s]
                };
                let (mo, md) = (to_model(p), to_model(d));
                for tri in indices.as_chunks::<3>().0 {
                    let a = &positions[tri[0] as usize * 3..tri[0] as usize * 3 + 3];
                    let b = &positions[tri[1] as usize * 3..tri[1] as usize * 3 + 3];
                    let cc = &positions[tri[2] as usize * 3..tri[2] as usize * 3 + 3];
                    if let Some(t) = ray_triangle(&mo, &md, a, b, cc) {
                        if t < limit {
                            limit = t;
                            best = Some(DecorHit {
                                cell,
                                band,
                                pos: words[lanes.pos_at(i)],
                                at: [base_x + lane(0, i), base_y + lane(1, i), lane(2, i)],
                                group: group as u32,
                                rotation: spin,
                                scale: lane(4, i),
                                t,
                            });
                        }
                    }
                }
            }
        }
    }
    best
}

fn ray_meets_square(o: &V3, d: &[f64; 3], x0: f64, y0: f64, x1: f64, y1: f64, limit: f64) -> bool {
    let square = Aabb {
        min: [x0, y0, f64::NEG_INFINITY],
        max: [x1, y1, f64::INFINITY],
    };
    square.span_along(*o, *d).is_some_and(|(enter, leave, _)| enter.max(0.0) <= leave.min(limit))
}

/// `p` is relative to the sphere's center.
fn ray_meets_sphere(p: &V3, d: &[f64; 3], r: f64, limit: f64) -> bool {
    let b = dot3(p, d);
    let c = dot3(p, p) - r * r;
    if c <= 0.0 {
        return true;
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return false;
    }
    let t = -b - disc.sqrt();
    t >= 0.0 && t <= limit
}

/// Möller–Trumbore, both sides.
fn ray_triangle(o: &[f64; 3], d: &[f64; 3], a: &[f32], b: &[f32], c: &[f32]) -> Option<f64> {
    let e1 = [b[0] as f64 - a[0] as f64, b[1] as f64 - a[1] as f64, b[2] as f64 - a[2] as f64];
    let e2 = [c[0] as f64 - a[0] as f64, c[1] as f64 - a[1] as f64, c[2] as f64 - a[2] as f64];
    let p = cross3(d, &e2);
    let det = dot3(&e1, &p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [o[0] - a[0] as f64, o[1] - a[1] as f64, o[2] - a[2] as f64];
    let u = dot3(&s, &p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross3(&s, &e1);
    let v = dot3(d, &q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = dot3(&e2, &q) * inv;
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_ray_down_the_z_axis_meets_the_triangle_under_it_at_its_height() {
        let t = ray_triangle(&[0.2, 0.2, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]);
        assert!(t.is_some_and(|t| (t - 4.0).abs() < 1e-12), "{t:?}");
        assert!(ray_triangle(&[0.2, 0.2, -5.0], &[0.0, 0.0, 1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_some());
        assert!(ray_triangle(&[0.8, 0.8, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]).is_none());
    }

    #[test]
    fn a_square_is_met_only_where_the_ray_passes_over_it() {
        let d = [0.6, 0.6, -0.3];
        assert!(ray_meets_square(&[0.0, 0.0, 10.0], &d, 5.0, 5.0, 20.0, 20.0, 100.0));
        assert!(!ray_meets_square(&[0.0, 0.0, 10.0], &d, 5.0, 5.0, 20.0, 20.0, 3.0));
        assert!(!ray_meets_square(&[0.0, 0.0, 10.0], &d, -20.0, 5.0, -5.0, 20.0, 100.0));
    }

    #[test]
    fn a_sphere_is_met_within_reach_and_missed_past_it() {
        assert!(ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 2.0, 0.0], &[1.0, 0.0, 0.0], 1.0, 100.0));
        assert!(!ray_meets_sphere(&[-10.0, 0.5, 0.0], &[1.0, 0.0, 0.0], 1.0, 5.0));
    }
}
