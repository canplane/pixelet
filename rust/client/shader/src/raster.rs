// pxl-shader/raster.rs

//! The samples' stops handed to the rasterizer (pxl-spec-core `raster`):
//! each stopped sample's box written for the raster's draw (`boxes_cs`),
//! which pxl-render raster.wgsl draws and pxl-shader cast.rs `cast_cs`
//! reads each pixel's pick off; and what lies about each box's face asked
//! once a box (`faces_cs`).

use pxl_spec_core::cast::{Ends, Met, Stop};
use pxl_spec_core::{beam, cast, raster, ways};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::memory::{Scope, Semantics};
use spirv_std::spirv;

use crate::add;
use crate::beam::{node_at, Beams};
use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, seen_put};
use crate::world::{Unlooked, World};

/// Whether samples at words `a` and `b` stopped on the same box, alike in
/// every word a pixel reads of them: the node, the block, whether it lacks
/// and the axis entered across, so what lies about its face is alike too
/// (world.rs `World::faced`, its face the axis's turned to the eye).
fn alike(beams: &Beams, a: u32, b: u32) -> bool {
    let mut k = 0;
    while k < beam::SAMPLE_FACES {
        if seen_at(beams, a + k) != seen_at(beams, b + k) {
            return false;
        }
        k += 1;
    }
    true
}

/// Word `i` of `s` set to `v`, none past it.
fn put(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        s[i as usize] = v;
    }
}

/// Word `i` of `s` raised to `v` where it is less, none past `s`.
fn most(s: &mut [u32], i: u32, v: u32) {
    if (i as usize) < s.len() {
        // SAFETY: just bounded by the slice's own length.
        unsafe {
            spirv_std::arch::atomic_u_max::<
                u32,
                { Scope::QueueFamily as u32 },
                { Semantics::NONE.bits() },
            >(s.get_unchecked_mut(i as usize), v)
        };
    }
}

/// Each sample of the screen (pxl-spec-core `beam::Grid::samples`), a
/// thread each: where it stopped, its box as the cast stops on it
/// (`cast::drawn`), appended to the boxes for the raster's draw, counted in
/// its args (`raster::ARGS_INSTANCES`), the dispatch that asks its face
/// raised to hold it (`raster::ARGS_FACES`), its number and its axis,
/// `raster::EYED` where it lies about the eye (`raster::eyed`); none where
/// it went on to its reach, or where the sample before it across or up
/// stopped alike (`alike`), which some sample before it in turn draws.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "boxes_cs"))]
pub fn boxes_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let g = grid(eye);
    if id.x >= g.samples[0] || id.y >= g.samples[1] {
        return;
    }
    let k = g.sample_at([id.x, id.y]);
    if seen_at(beams, k + beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE {
        return;
    }
    if id.x > 0 && alike(beams, k, g.sample_at([id.x - 1, id.y])) {
        return;
    }
    if id.y > 0 && alike(beams, k, g.sample_at([id.x, id.y - 1])) {
        return;
    }
    let (origin, _) = eye_origin(eye);
    let node = node_at(beams, k + beam::SAMPLE_NODE);
    let (lo, hi) = cast::drawn(origin, &node, seen_at(beams, k + beam::SAMPLE_BLOCK));
    let lacks = seen_at(beams, k + beam::SAMPLE_LACKS) != 0;
    let eyed = raster::eyed(lo, hi, [eye.tan.x, eye.tan.y]);
    let box_at = add(args, raster::ARGS_INSTANCES, 1);
    most(args, raster::ARGS_FACES, box_at / raster::FACES_LANES + 1);
    let at = box_at * raster::BOX_WORDS;
    put(boxes, at + raster::BOX_LO, lo[0].to_bits());
    put(boxes, at + raster::BOX_LO + 1, lo[1].to_bits());
    put(boxes, at + raster::BOX_LO + 2, lo[2].to_bits());
    put(boxes, at + raster::BOX_HI, hi[0].to_bits());
    put(boxes, at + raster::BOX_HI + 1, hi[1].to_bits());
    put(boxes, at + raster::BOX_HI + 2, hi[2].to_bits());
    put(
        boxes,
        at + raster::BOX_ID,
        raster::id_of(id.y * g.samples[0] + id.x, lacks),
    );
    put(
        boxes,
        at + raster::BOX_AXIS,
        seen_at(beams, k + beam::SAMPLE_AXIS) | if eyed { raster::EYED } else { 0 },
    );
}

/// Each box `boxes_cs` wrote, a thread each, `raster::FACES_LANES` a
/// workgroup, dispatched off its args (`raster::ARGS_FACES`), each past the
/// boxes' count returning at once: what lies about the face its sample
/// entered (`World::faced`), asked of the world's query once a box rather
/// than once a sample, and left in its sample's words
/// (`beam::SAMPLE_FACES`), the one sample of the box a pixel reads
/// (cast.rs `cast_cs`). Its sample's ray is the one `sample_cs` cast, and
/// its stop the node, block and axis it left; a sample that lacks met no
/// face.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "faces_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn faces_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &[u32],
) {
    if id.x >= args[raster::ARGS_INSTANCES as usize] {
        return;
    }
    let box_id = boxes[(id.x * raster::BOX_WORDS + raster::BOX_ID) as usize];
    let g = grid(eye);
    let s = raster::sample_of(box_id);
    let sample = [s % g.samples[0], s / g.samples[0]];
    let k = g.sample_at(sample);
    let faces = if box_id & raster::LACK != 0 {
        0
    } else {
        let (origin, _) = eye_origin(eye);
        let ray = eye_ray(eye, g.sample(sample));
        let met = Met {
            ends: Ends::Stop,
            stop: Stop {
                t: 0.0,
                axis: seen_at(beams, k + beam::SAMPLE_AXIS),
                node: node_at(beams, k + beam::SAMPLE_NODE),
                at: 0u32,
                block: [
                    seen_at(beams, k + beam::SAMPLE_BLOCK),
                    seen_at(beams, k + beam::SAMPLE_BLOCK + 1),
                ],
            },
            visits: 0,
        };
        let mut world = World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        world.faced(origin, &ray, &met)
    };
    seen_put(beams, k + beam::SAMPLE_FACES, faces);
}
