//! The cast's compute entry (`cast_cs`): each camera-grid cell's ray down
//! the pyramid by pxl-portable's descent, the one the CPU's cast runs, over
//! the world as the GPU holds it. The harness's entries are behind its
//! feature (`harness`).

#![cfg_attr(target_arch = "spirv", no_std)]

#[cfg(feature = "harness")]
pub mod harness;

use pxl_portable::cast::{self, Detail, Ends, In, Kind, Met, Query, Until};
use pxl_portable::caves::Cave;
use spirv_std::glam::{IVec4, UVec2, UVec3, UVec4, Vec4};
use spirv_std::spirv;

/// The eye a cast is from (pxl-kernel view/sight.rs `gpu_cast`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Eye {
    /// The Q3 position, and a pad.
    pub origin: IVec4,
    /// The unit forward, and the reach.
    pub forward_far: Vec4,
    /// The unit right, and τ.
    pub right_tau: Vec4,
    /// The unit up, and a pad.
    pub up: Vec4,
    /// The cells across and up, and the zooms a block is asked at.
    pub across_range: UVec4,
}

/// A node as the GPU's descent reaches it: its zoom and index, and its caves.
#[derive(Clone, Copy)]
pub struct At {
    zoom: u32,
    node: [i32; 3],
    cave: Cave,
}

/// The world as the GPU holds it: the caves alone, for now.
pub struct World;

impl Query for World {
    type At = At;
    type Block = u32;
    const AIR: u32 = 0;

    fn root(&mut self, node: [i32; 3]) -> At {
        At {
            zoom: 0,
            node,
            cave: Cave::of(0, node),
        }
    }

    fn child(&mut self, at: &At, k: u32) -> At {
        let z = at.zoom + 1;
        let node = [2 * at.node[0] + (k & 1) as i32, 2 * at.node[1] + (k >> 1 & 1) as i32, 2 * at.node[2] + (k >> 2 & 1) as i32];
        At {
            zoom: z,
            node,
            cave: at.cave.child(z, node),
        }
    }

    fn node(&mut self, at: &mut At, [_, near]: [u32; 2], _t0: f32) -> In<u32> {
        if at.zoom >= near {
            return In {
                kind: Kind::Block,
                block: if at.cave.hollow() { 0 } else { 1 },
            };
        }
        match at.cave.whole(at.zoom) {
            (true, true) => In { kind: Kind::Block, block: 0 },
            _ => In { kind: Kind::Split, block: 0 },
        }
    }

    fn fills(&self, block: &u32) -> bool {
        *block != 0
    }
}

/// The lane's word for where a ray stopped: the value, its kind (ground) and
/// the face it entered through, then how far, as f32 bits; nothing met, zero.
fn word(ray: &cast::Ray, met: Met<At, u32>) -> UVec2 {
    if met.ends != Ends::Stop {
        return UVec2::new(0, 0);
    }
    let s = met.stop;
    let down = ray.direction[s.axis as usize] > 0.0;
    let face = 2 * s.axis + down as u32;
    UVec2::new(s.block | 1 << 16 | face << 18, s.t.to_bits())
}

#[spirv(compute(threads(8, 8, 1)))]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] lane: &mut [UVec2],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] tangents: &[f32],
) {
    let across = eye.across_range;
    if id.x >= across.x || id.y >= across.y {
        return;
    }
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let direction = cast::through(v3(eye.forward_far), v3(eye.right_tau), v3(eye.up), tangents[id.x as usize], tangents[(across.x + id.y) as usize]);
    let ray = cast::Ray::of([eye.origin.x, eye.origin.y, eye.origin.z], direction, eye.forward_far.w);
    let detail = Detail {
        tau: eye.right_tau.w,
        range: [across.z, across.w],
    };
    let met = cast::first(&mut World, &ray, detail, Until::Fills);
    lane[(1 + id.y * across.x + id.x) as usize] = word(&ray, met);
}
