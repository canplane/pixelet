// pxl-kernel/layer/decor/hit.rs

//! Which decor the pointer's ray strikes: the ray against the triangles of
//! every instance the z16 decor cells hold (layer/decor/cell.rs), from the
//! same blocks the renderer draws. A decor paints no voxel, so the access's
//! ray (world/access.rs `first_hit`) passes through it; this is its own hit,
//! against what is drawn. What comes back is what an edit acts on: a placed
//! instance's entry, which the object store files it under, or the cell a
//! scattered one stands in, which has no entry and is written over as the
//! ground is.

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::indexer::L2_IDX;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::{Aabb, V3};
use xpute_runtime::mem::base::ptr_at;

use crate::kernel::kernel;
use crate::layer::decor::asset_store::decor_bundle_assets;
use crate::layer::decor::instance::{decor_groups, GroupLanes, NO_ENTRY};
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::EntryAt;
use crate::world::chunk::table::Scheduler;

// A decor mesh sitting on the ground it also intersects (any tree's own
// trunk-base, by construction) has its own hit distance land almost exactly AT
// max_dist, not comfortably inside it — the two distances come from two
// independent algorithms (a triangle test against the model here, the
// access's walk for max_dist), so they only ever agree up to float noise. A
// strict reject flipped on essentially every frame right at that boundary as a
// real mouse's sub-pixel jitter walked the disagreement across zero, which read
// as the hover ghost flickering while the cursor sat still. 1e-3
// units is small enough never to let a decor mesh that is genuinely occluded by
// terrain win, and larger than the round-off.
const SAME_SURFACE_EPS: f64 = 1e-3;

/// What decor a ray struck.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecorStruck {
    /// A placed instance: where its entry is filed.
    Placed(EntryAt),
    /// A scattered instance.
    Scattered(Scattered),
}

/// A scattered instance a ray struck: the cell it stands in, which a leaf
/// written there overrides, and how it is drawn — its group, where it
/// stands, its turn and its size — for a preview drawn as it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scattered {
    pub cell: [i64; 3],
    pub group: u32,
    pub at: [f64; 3],
    pub rotation: f64,
    pub scale: f64,
}

/// The decor the ray strikes within `max_dist`, or None. The ray is in the
/// root frame, relative to `map_origin`.
pub fn raycast_decor(origin: V3, direction: V3, map_origin: IVec, max_dist: f64) -> Option<DecorStruck> {
    let (_, sched) = kernel().world()?;
    let frame = [ivec::to_real(ivec::X(map_origin)), ivec::to_real(ivec::Y(map_origin))];
    let hit = hit_decor(sched, frame, origin, direction, max_dist + SAME_SURFACE_EPS)?;
    if hit.band == NO_ENTRY {
        return Some(DecorStruck::Scattered(Scattered {
            cell: hit.at.map(|v| (v / CELL.side()).floor() as i64),
            group: hit.group,
            at: hit.at,
            rotation: hit.rotation,
            scale: hit.scale,
        }));
    }
    // The lane holds the band's own i32 as a word (layer/decor/build.rs).
    Some(DecorStruck::Placed(EntryAt {
        col_idx: {
            let (x0, y0, _) = hit.cell.rect();
            L2_IDX(ivec::gather(x0 as i64, y0 as i64, 0))
        },
        band_idx: hit.band as i32,
        pos: hit.pos,
    }))
}

/// What a ray struck among the decor shown: the decor cell it stands in, a
/// z16 one, the entry's band and packed position in that cell's column —
/// NO_ENTRY for a scattered instance — where the instance stands, and how
/// far along.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DecorHit {
    pub cell: crate::layer::decor::cell::DecorCell,
    pub band: u32,
    pub pos: u32,
    pub at: [f64; 3],
    pub group: u32,
    pub rotation: f64,
    pub scale: f64,
    pub t: f64,
}

/// The model each decor group is drawn with, in group order, none while it
/// loads.
fn group_models() -> impl Iterator<Item = Option<u32>> {
    decor_bundle_assets().iter().flat_map(|e| e.models.iter().copied())
}

/// How far past a decor cell's own square a placed instance standing in it can
/// reach, in units: the largest decor model's bounding radius at the largest
/// size decor is placed at — what the models are, and nothing guessed — only
/// for skipping cells the ray never comes near.
fn decor_reach(sched: &Scheduler) -> f64 {
    let radius = group_models()
        .flatten()
        .filter_map(|m| sched.models.mesh(m))
        .map(|m| m.radius as f64 * m.scale as f64)
        .fold(0.0, f64::max);
    radius * DECOR_SIZE
}

/// The nearest decor instance the ray strikes within `max_dist`, over the
/// decor every shown z16 cell holds, model triangle by triangle after a
/// bounding sphere. Hidden ones are tested, since the hidden one is what the
/// eraser is over.
fn hit_decor(sched: &Scheduler, origin: [f64; 2], o: V3, d: [f64; 3], max_dist: f64) -> Option<DecorHit> {
    let mut best: Option<DecorHit> = None;
    let mut limit = max_dist;
    let reach = decor_reach(sched);
    for (cell, base_x, base_y, words) in crate::layer::decor::cell::shown() {
        // Only a z16 cell names its placed entries, and what an edit acts on
        // is made at z16: what is erased is erased from close by.
        if cell.level != 0 {
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
            // SAFETY: the model's block, positions first and the indices
            // past the vertex words.
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
                let p = [o[0] - c[0], o[1] - c[1], o[2] - c[2]];
                if !ray_meets_sphere(&p, &d, r, limit) {
                    continue;
                }
                // Into the model's own frame: the spin undone, then the
                // quarter turn about X that stood it up, then the scale.
                let (sn, cs) = (-spin).sin_cos();
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

/// Whether the ray crosses the square [x0, x1] × [y0, y1] at any height
/// within `limit`.
fn ray_meets_square(o: &V3, d: &[f64; 3], x0: f64, y0: f64, x1: f64, y1: f64, limit: f64) -> bool {
    let square = Aabb {
        min: [x0, y0, f64::NEG_INFINITY],
        max: [x1, y1, f64::INFINITY],
    };
    square.span_along(*o, *d).is_some_and(|(enter, leave, _)| enter.max(0.0) <= leave.min(limit))
}

/// Whether the ray from `p` (relative to the sphere's center) comes within
/// `r` of it before `limit`.
fn ray_meets_sphere(p: &V3, d: &[f64; 3], r: f64, limit: f64) -> bool {
    let b = p[0] * d[0] + p[1] * d[1] + p[2] * d[2];
    let c = p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - r * r;
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

/// Möller–Trumbore, both sides: the distance along `d` to the triangle.
fn ray_triangle(o: &[f64; 3], d: &[f64; 3], a: &[f32], b: &[f32], c: &[f32]) -> Option<f64> {
    let e1 = [b[0] as f64 - a[0] as f64, b[1] as f64 - a[1] as f64, b[2] as f64 - a[2] as f64];
    let e2 = [c[0] as f64 - a[0] as f64, c[1] as f64 - a[1] as f64, c[2] as f64 - a[2] as f64];
    let p = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
    let det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [o[0] - a[0] as f64, o[1] - a[1] as f64, o[2] - a[2] as f64];
    let u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
    let v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) * inv;
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_ray_down_the_z_axis_meets_the_triangle_under_it_at_its_height() {
        let t = ray_triangle(&[0.2, 0.2, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 1.0], &[1.0, 0.0, 1.0], &[0.0, 1.0, 1.0]);
        assert!(t.is_some_and(|t| (t - 4.0).abs() < 1e-12), "{t:?}");
        // From the other side too, and not beside it.
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
