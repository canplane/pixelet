// pxl-shader/raster.rs

//! The samples' stops handed to the rasterizer (pxl-spec-core `raster`):
//! each stopped sample's box written for the raster's draw (`boxes_cs`),
//! which pxl-render raster.wgsl draws and pxl-shader cast.rs `cast_cs`
//! reads each pixel's pick off; and what lies about each box's face asked
//! once a box (`faces_cs`).

use pxl_spec_core::cast::{Ends, Met, Stop};
use pxl_spec_core::{beam, cast, raster, support, way};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::beam::{node_at, Beams, Words};
use crate::either::Either;
use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::observed::{seen_at, seen_put};
use crate::observer::{self, Observer};
use crate::sun::Moved;
use crate::world::{Unlooked, World};
use crate::{add, most};

/// Whether samples at words `a` and `b` stopped on the same box, alike in
/// every word a pixel reads of them: the node, the block, whether it lacks
/// and the axis entered across, so what lies about its face is alike too
/// (world.rs `World::faced`, its face the axis's turned to the eye).
fn alike<W: Words + ?Sized>(w: &W, a: u32, b: u32) -> bool {
    let mut k = 0;
    while k < beam::SAMPLE_FACES {
        if w.word(a + k) != w.word(b + k) {
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

/// No sample before it (`boxed`).
pub(crate) const NONE: u32 = u32::MAX;

/// The sample at word `k` of `w`, numbered `sample`, written as a box for
/// the raster's draw, the eye's and the sun's alike: its box as the cast
/// stops on it (`cast::drawn`) from `origin`, appended to `boxes`, counted
/// in `args` (`raster::ARGS_INSTANCES`), its number and its axis,
/// `raster::EYED` where it lies about the eye at the screen's tangents
/// `tan` (`raster::eyed`), where `eyed` asks (the sun's has none); none
/// where it went on to its reach, or where a sample at `before`, the one
/// across or up, `NONE` where there is none, stopped alike (`alike`), which
/// some sample before it in turn draws. Its place among the boxes, `NONE`
/// where it wrote none.
pub(crate) fn boxed<W: Words + ?Sized>(
    w: &W,
    k: u32,
    before: [u32; 2],
    origin: [i32; 3],
    sample: u32,
    (eyed, tan): (bool, [f32; 2]),
    boxes: &mut [u32],
    args: &mut [u32],
) -> u32 {
    if w.word(k + beam::SAMPLE_NODE + beam::NODE_ZOOM) == beam::SAMPLE_NONE {
        return NONE;
    }
    if before[0] != NONE && alike(w, k, before[0]) {
        return NONE;
    }
    if before[1] != NONE && alike(w, k, before[1]) {
        return NONE;
    }
    let node = node_at(w, k + beam::SAMPLE_NODE);
    let (lo, hi) = cast::drawn(origin, &node, w.word(k + beam::SAMPLE_BLOCK));
    let lacks = w.word(k + beam::SAMPLE_LACKS) != 0;
    let eyed = eyed && raster::eyed(lo, hi, tan);
    let box_at = add(args, raster::ARGS_INSTANCES, 1);
    let at = box_at * raster::BOX_WORDS;
    put(boxes, at + raster::BOX_LO, lo[0].to_bits());
    put(boxes, at + raster::BOX_LO + 1, lo[1].to_bits());
    put(boxes, at + raster::BOX_LO + 2, lo[2].to_bits());
    put(boxes, at + raster::BOX_HI, hi[0].to_bits());
    put(boxes, at + raster::BOX_HI + 1, hi[1].to_bits());
    put(boxes, at + raster::BOX_HI + 2, hi[2].to_bits());
    put(boxes, at + raster::BOX_ID, raster::id_of(sample, lacks));
    put(
        boxes,
        at + raster::BOX_AXIS,
        w.word(k + beam::SAMPLE_AXIS) | if eyed { raster::EYED } else { 0 },
    );
    box_at
}

/// The block a ray from `origin` takes off the raster's pick, the sample at
/// word `k` of `w`: where the ray enters it (`cast::entry`) across the axis
/// the sample entered by, the eye's pixels and the sun's slots alike, and
/// whether it does.
pub(crate) fn picked<W: Words + ?Sized>(
    w: &W,
    k: u32,
    origin: [i32; 3],
    ray: &cast::Ray,
) -> (cast::Stop<u32>, bool) {
    let node = node_at(w, k + beam::SAMPLE_NODE);
    let block = [
        w.word(k + beam::SAMPLE_BLOCK),
        w.word(k + beam::SAMPLE_BLOCK + 1),
    ];
    let (enter, axis, enters) = cast::entry(origin, ray, &node, block[0]);
    let stop = cast::Stop {
        t: enter,
        axis,
        node,
        at: 0u32,
        block,
    };
    (stop, enters && axis == w.word(k + beam::SAMPLE_AXIS))
}

/// Each sample of its observer's lattice, a thread each: written as a box
/// for the raster's draw (observer.rs `sample_box`), the screen's or the
/// sun's map's as its head names (`Either`); and, the screen's, the
/// dispatch that asks its face raised to hold it (`raster::ARGS_FACES`),
/// as its pixels draw the face.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sample_box_cs"))]
pub fn sample_box_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let o = Either::of(eye, moved);
    let [across, down] = o.samples();
    if id.x >= across || id.y >= down {
        return;
    }
    let box_at = observer::sample_box(&o, beams, [id.x, id.y], boxes, args);
    if !o.sun() && box_at != NONE {
        most(args, raster::ARGS_FACES, box_at / raster::FACES_LANES + 1);
    }
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
        world.faced(origin, &ray, &met, support::line)
    };
    seen_put(beams, k + beam::SAMPLE_FACES, faces);
}
