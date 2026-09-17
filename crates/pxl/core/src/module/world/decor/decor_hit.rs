// pxl_core/module/world/decor/decor_hit.rs

//! Which user-placed decor entry the pointer's ray strikes, by the kernel's hit
//! test (crates/pxl/core/src/render/layer/pipeline/chunk_raycast.rs's hit_decor): the ray against the
//! triangles of every placed instance the on-screen chunks show, from the same
//! blocks the renderer draws. What comes back is the chunk it stands in and the
//! entry's band and packed position in that column, which is the address the
//! object store files the entry under.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::mat4::Vec3;

use crate::sys::kernel::kernel;
use crate::module::world::object::store::EntryAt;
use crate::render::layer::pipeline::chunk_raycast::{ray_hit_decor, RayLike};

// A decor mesh sitting on the ground it also intersects (any tree's own
// trunk-base, by construction) has its own hit distance land almost exactly AT
// max_dist, not comfortably inside it — the two distances come from two
// independent algorithms (a triangle test against the model here, the voxel
// walk's slab test for max_dist), so they only ever agree up to float noise. A
// strict reject flipped on essentially every frame right at that boundary as a
// real mouse's sub-pixel jitter walked the disagreement across zero, which read
// as the hover ghost flickering while the cursor sat still (2026-09-05). 1e-3
// units is small enough never to let a decor mesh that is genuinely occluded by
// terrain win, and larger than the round-off.
const SAME_SURFACE_EPS: f64 = 1e-3;

/// Where the placed decor entry the ray strikes within `max_dist` is filed, or
/// None.
/// Procedural instances are passed over by the kernel, so a hit on one falls
/// through to whatever is next along the ray. The ray is in the root frame,
/// relative to `map_origin`, as the voxel raycast takes it.
pub fn raycast_decor(origin: Vec3, direction: Vec3, map_origin: IVec, max_dist: f64) -> Option<EntryAt> {
    let hit = ray_hit_decor(&RayLike { origin, direction }, map_origin, max_dist + SAME_SURFACE_EPS)?;
    let (_, col_idx, ..) = kernel().sched.builder.source_of(hit.chunk)?;
    // The lane holds the band's own i32 as a word (world/decor.rs), NO_ENTRY
    // apart, which the kernel has already passed over.
    Some(EntryAt {
        col_idx,
        band_idx: hit.band as i32,
        pos: hit.pos,
    })
}
