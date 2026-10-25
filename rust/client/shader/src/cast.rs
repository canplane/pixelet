// pxl-shader/cast.rs

//! Each drawn pixel's ray (`cast_cs`) off the samples about it or down the
//! observed pyramid, and those that missed it walked again (`miss_cs`).

use pxl_spec_core::cast::{self, Ends, Met};
use pxl_spec_core::{beam, observed, ray, ways};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::{spirv, Image};

use crate::beam::{node_at, resumed, started, Beams};
use crate::eye::{eye_origin, grid, Eye};
use crate::observed::{seen_at, Reading, Seen, Writing, MISSED};
use crate::world::{face_of, Unlooked, World};

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-spec-core `ray`): the cast writes them.
type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four looks
/// the ground's block is drawn by, a byte each (`ground::word`), or in the
/// steps view the nodes its descent asked about, then what lies about its
/// face (`World::faced`); where it met nothing, the first two zero.
fn word<A: Copy>(
    ray: &cast::Ray,
    met: Met<A>,
    steps: bool,
    faces: u32,
) -> [u32; ray::WORDS as usize] {
    if met.ends != Ends::Stop {
        return [0, 0, met.visits, 0];
    }
    let s = met.stop;
    let [value, four] = s.block;
    [
        ray::first_word(value, face_of(ray, s.axis), s.node.zoom),
        s.t.to_bits(),
        if steps { met.visits } else { four },
        faces,
    ]
}
const _: () = assert!(
    ray::FACES == 3,
    "what lies about a ray's face is its last word"
);

/// The tangents across and up of the middle of drawn pixel `p` of the
/// `size` the rays are, row 0 at the top, as the screen pass forms its own
/// pixel's: the screen's half-angles' scaled by where it lies.
fn pixel_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5) / size.y as f32 * 2.0;
    [ndc_x * eye.tan.x, ndc_y * eye.tan.y]
}

/// A ray's words written into the rays at pixel `p`.
fn written(rays: &Rays, p: UVec3, w: [u32; ray::WORDS as usize]) {
    let mut k = 0;
    while k < ray::WORDS {
        // SAFETY: within the rays, bounded by their size where `p` was.
        unsafe { rays.write(UVec3::new(p.x, p.y, k), w[k as usize]) };
        k += 1;
    }
}

/// Each pixel's ray: the nearest of the blocks the samples about it stopped
/// at that it enters (pxl-spec-core `beam::Grid::about`, `cast::entry`), its
/// words, no step its own; where it enters none, or a node a sample lacks
/// at before it, the ray from its cell's start, reading the
/// observed pyramid alone (`Reading`), its binding read-only so no read of
/// it is atomic: its words where every node it asked was kept, else its
/// first word the mark `miss_cs` walks it again by (pxl-spec-core
/// `ray::MISS`). A block a sample stopped at comes with what lies about
/// its face, the sample's; a pixel that walked itself asks the world's
/// query its own (`World::faced`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cast_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    // Read-write as the group lays them out, which WebGPU holds a shader's
    // own access to; no atomic touches them here, so their reads stay
    // plain.
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (seen, beams, window): (&Seen, &Beams, &[IVec4]) = (seen, beams, window);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let about = g.about(px);
    // The nearest block entered, at the reach where none is.
    let mut hit = cast::Stop {
        t: ray.reach,
        axis: 0,
        node: cast::Node::ROOT,
        at: 0u32,
        block: MISSED,
    };
    // What lies about its face, the sample's.
    let mut faces = 0;
    // The nearest node a sample lacks at that the ray enters.
    let mut lack = ray.reach;
    let mut j = 0;
    while j < 2 * beam::NEAR {
        let mut i = 0;
        while i < 2 * beam::NEAR {
            let k = g.sample_at([about[0] + i, about[1] + j]);
            if seen_at(beams, k + beam::SAMPLE_NODE + beam::NODE_ZOOM) != beam::SAMPLE_NONE {
                let node = node_at(beams, k + beam::SAMPLE_NODE);
                let block = [
                    seen_at(beams, k + beam::SAMPLE_BLOCK),
                    seen_at(beams, k + beam::SAMPLE_BLOCK + 1),
                ];
                let (enter, axis, enters) = cast::entry(origin, &ray, &node, block[0]);
                let lacks = seen_at(beams, k + beam::SAMPLE_LACKS) != 0;
                // A block's face the pixel enters across another axis than its
                // sample did lies under what the sample passed: not the
                // pixel's first, so it is no candidate.
                let exposed = axis == seen_at(beams, k + beam::SAMPLE_AXIS);
                if enters && lacks && enter < lack {
                    lack = enter;
                } else if enters && !lacks && exposed && enter < hit.t {
                    hit = cast::Stop {
                        t: enter,
                        axis,
                        node,
                        at: 0u32,
                        block,
                    };
                    faces = seen_at(beams, k + beam::SAMPLE_FACES);
                }
            }
            i += 1;
        }
        j += 1;
    }
    let steps = eye.origin.w != 0;
    if hit.t < ray.reach && hit.t < lack {
        written(
            rays,
            id,
            word(
                &ray,
                Met {
                    ends: Ends::Stop,
                    stop: hit,
                    visits: 0,
                },
                steps,
                faces,
            ),
        );
        return;
    }
    let met = resumed(&mut Reading { seen }, origin, &ray, detail, t, &line);
    if met.ends == Ends::Lacks && met.stop.block[0] == MISSED[0] {
        // SAFETY: within the rays, bounded by their size above.
        unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
    } else {
        let mut world = World {
            window,
            ground,
            looks: Unlooked(0),
            shards,
            overwrites,
            eye,
            pool: ways::tree::Read(pool),
        };
        written(
            rays,
            id,
            word(&ray, met, steps, world.faced(origin, &ray, &met)),
        );
    }
}

/// Each pixel `cast_cs` marked missed, its ray again from its cell's
/// start, writing what it asks into the observed pyramid (`Writing`), and
/// what lies about the face it stopped on (`World::faced`): a thread a
/// drawn pixel, as the cast's, each but the marked returning at once. It
/// runs after `cast_cs` in the cast's one pass and writes every marked
/// pixel's words whole, so the mark never outlives the pass.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn miss_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] looks: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let (window, beams): (&[IVec4], &Beams) = (window, beams);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let (origin, detail) = eye_origin(eye);
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let (ray, t, line) = started(eye, &g, beams, g.cell(px), px);
    let tag = observed::tag(seen_at(seen, observed::FRAME), 2);
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let met = resumed(
        &mut Writing { world, seen, tag },
        origin,
        &ray,
        detail,
        t,
        &line,
    );
    written(
        rays,
        id,
        word(
            &ray,
            met,
            eye.origin.w != 0,
            world.faced(origin, &ray, &met),
        ),
    );
}
