// pxl-shader/cast.rs

//! Each drawn pixel's ray (`cast_cs`) off the cell the raster kept at it
//! or the sky, and the rest walked (`miss_list_cs` over their list,
//! `miss_cs` over every pixel where the list was full).

use pxl_spec_core::cast::{self, Ends, Met};
use pxl_spec_core::{beam, observed, ray, support, tally, way};
use spirv_std::glam::{IVec4, UVec3, UVec4};

use spirv_std::{spirv, Image};

use crate::beam::Beams;
use crate::eye::{grid, Eye};
use crate::observed::{seen_at, seen_put, Lined, Seen};
use crate::observer::{self, Observer, Takes};
use crate::world::{face_of, World};
use crate::{add, most};

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-spec-core `ray`): the cast writes them.
pub(crate) type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The raster's pick at each drawn pixel (pxl-render gpu/ray/raster.rs): its
/// sample's number (pxl-spec-core `raster::id_of`), 0 where it kept none,
/// read as an array of one layer.
type Picks = Image!(2D, type = u32, sampled = true, arrayed = true);

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four looks
/// the ground's asset is drawn by, a byte each (`ground::word`), or in the
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
    let [value, four] = s.asset;
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
pub(crate) fn pixel_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5) / size.y as f32 * 2.0;
    [ndc_x * eye.tan.x, ndc_y * eye.tan.y]
}

/// A ray's words written into the rays at pixel `p`.
fn write_ray(rays: &Rays, p: UVec3, w: [u32; ray::WORDS as usize]) {
    let mut k = 0;
    while k < ray::WORDS {
        // SAFETY: within the rays, bounded by their size where `p` was.
        unsafe { rays.write(UVec3::new(p.x, p.y, k), w[k as usize]) };
        k += 1;
    }
}

/// `cast_cs`'s workgroup a side, a square of pixels (pxl-tool check_rules.rs
/// `WORKGROUPS`).
const CAST_SIDE: u32 = 8;

/// Each pixel's ray: the cell the raster kept at it (pxl-spec-core
/// `raster`; pxl-render gpu/ray/raster.rs), the nearest a sample stopped at
/// that its ray enters across the axis that sample entered by, tested again
/// exactly (`cast::entry`) where a GPU's f32 may part the two, its words, no
/// step its own, with what lies about its face, its sample's; the sky
/// where no sample about it met anything. Any other pixel, where it kept a
/// node a sample lacks at, or none, or the test refuses it, takes no walk
/// here: its first word is the mark the walk again goes by (pxl-spec-core
/// `ray::MISS`; `miss_list_cs`, `miss_cs`), so no lane carries a walk's
/// footprint for the few that walk. A marked pixel is listed for that walk
/// (pxl-spec-core `observed::MARKS_AT`) by a plain write, its place taken
/// off the count in its dispatch's words (`marks`, `ray::MISS_ARGS_COUNT`),
/// so no word of the pyramid is typed atomic here; one past the list's room
/// has the walk over every pixel dispatched instead.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "cast_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
    #[spirv(descriptor_set = 0, binding = 13)] picks: &Picks,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] marks: &mut [u32],
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let g = grid(eye);
    let px = pixel_tan(eye, id, size);
    let aim = eye.aim(px);
    let ray = aim.ray;
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = picks.fetch(UVec3::new(id.x, id.y, 0));
    // The cell off its pick, the sky, or a walk of its own (observer.rs
    // `resolve`), the nearest cell entered at the reach where none is.
    let (took, hit, k) = observer::resolve(eye, beams, px, &aim, kept.x);
    let steps = eye.origin.w != 0;
    let on = seen_at(seen, observed::TALLY_ON) != 0;
    // No sample about it met anything: the sky, taken without a walk of its
    // own, as a thing thinner than the samples' spacing between them goes
    // unseen.
    if took == Takes::Sky {
        write_ray(
            rays,
            id,
            word(
                &ray,
                Met {
                    ends: Ends::On,
                    stop: hit,
                    visits: if steps {
                        seen_at(
                            beams,
                            g.sample_at(g.sample_before(px)) + beam::SAMPLE_VISITS,
                        )
                    } else {
                        0
                    },
                },
                steps,
                0,
            ),
        );
        return;
    }
    if took == Takes::Pick {
        // What lies about its face, the sample's, and the nodes the
        // sample's walk visited, which the steps view shows as this pixel's.
        let (faces, visits) = (
            seen_at(beams, k + beam::SAMPLE_FACES),
            seen_at(beams, k + beam::SAMPLE_VISITS),
        );
        add_count(marks, on, tally::PICKED, 1);
        write_ray(
            rays,
            id,
            word(
                &ray,
                Met {
                    ends: Ends::Stop,
                    stop: hit,
                    visits,
                },
                steps,
                faces,
            ),
        );
        return;
    }
    // Neither: marked and listed for its walk (`miss_list_cs`), which reads
    // and writes the pyramid. No walk is this pass's own, so a lane that
    // takes its pick pays none of a walk's footprint.
    add_count(marks, on, tally::WALKED, 1);
    // SAFETY: within the rays, bounded by their size above.
    unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
    let whole = [size.x.div_ceil(CAST_SIDE), size.y.div_ceil(CAST_SIDE)];
    let at = enlist(marks, observed::MARKS, whole);
    if at < observed::MARKS {
        seen_put(seen, observed::MARKS_AT + at, id.y << 16 | id.x);
    }
}

/// `v` added to lane `lane` of a pixel's or a slot's (pxl-spec-core
/// `tally::PICKED` on) in its walk again's args `args`, where `on`
/// (`ray::MISS_ARGS_TALLY`), the eye's pixels and the sun's slots alike.
pub(crate) fn add_count(args: &mut [u32], on: bool, lane: u32, v: u32) {
    if on {
        add(args, ray::MISS_ARGS_TALLY + lane - tally::PICKED, v);
    }
}

/// One more missed ray listed for its walk again, the eye's pixels and the
/// sun's slots alike (pxl-shader sun.rs `sun_cs`): its place off the count
/// in its dispatch's words `args` (pxl-spec-core `ray::MISS_ARGS_COUNT`),
/// the rows the list fills raised to hold it (`ray::MISS_ARGS_LIST`); one
/// past the list's `room` has the walk over `whole` workgroups dispatched
/// instead (`ray::MISS_ARGS_WHOLE`). Its place, which the caller writes
/// where it is under `room`.
pub(crate) fn enlist(args: &mut [u32], room: u32, whole: [u32; 2]) -> u32 {
    let at = add(args, ray::MISS_ARGS_COUNT, 1);
    if at < room {
        most(
            args,
            ray::MISS_ARGS_LIST + 1,
            at / (ray::MISS_LANES * ray::MISS_ACROSS) + 1,
        );
    } else {
        most(args, ray::MISS_ARGS_WHOLE, whole[0]);
        most(args, ray::MISS_ARGS_WHOLE + 1, whole[1]);
    }
    at
}

/// Pixel `id`'s ray again from its cell's start, writing what it asks into
/// the observed pyramid (`Writing`), and what lies about the face it
/// stopped on, asked off the line the walk kept (`World::faced`): its
/// words.
#[allow(clippy::too_many_arguments)]
fn missed(
    eye: &Eye,
    window: &[IVec4],
    ground: &[u32],
    looks: &[u32],
    shards: &[IVec4],
    overwrites: &[u32],
    pool: &[u32],
    beams: &Beams,
    seen: &mut Seen,
    id: UVec3,
    size: UVec3,
) -> [u32; ray::WORDS as usize] {
    let mut world = World {
        window,
        ground,
        looks,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let mut lined = Lined::before();
    let (aim, met, before) =
        observer::walked(eye, world, seen, beams, pixel_tan(eye, id, size), &mut lined);
    let faces = world.faced(aim.origin, &aim.ray, &met, |_, _, _| {
        support::Line::kept(aim.origin, &met.stop.node, lined.live, &lined.stack)
    });
    let met = Met {
        visits: met.visits + before,
        ..met
    };
    word(&aim.ray, met, eye.origin.w != 0, faces)
}

/// Each pixel `cast_cs` marked missed and could not list, its ray again
/// (`missed`): a thread a drawn pixel, each but the marked returning at
/// once, dispatched only where the list was full (`ray::MISS_ARGS_WHOLE`),
/// after the list's walk, so a listed pixel it finds is no longer marked.
/// The two run after `cast_cs` in the cast's one pass and write every
/// marked pixel's words whole, so the mark never outlives the pass.
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
    let w = missed(
        eye, window, ground, looks, shards, overwrites, pool, beams, seen, id, size,
    );
    write_ray(rays, id, w);
}

/// Each pixel `cast_cs` listed (pxl-spec-core `observed::MARKS_AT`), its
/// ray again (`missed`): a thread a listed pixel, `ray::MISS_LANES` a
/// workgroup and `ray::MISS_ACROSS` workgroups a row, dispatched over the
/// rows the list filled (`ray::MISS_ARGS_LIST`), each past the count
/// returning at once. The count is copied into the pyramid's buffer once
/// `cast_cs` is done (`observed::MARKS_COUNT`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "miss_list_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn miss_list_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
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
    let k = (group.y * ray::MISS_ACROSS + group.x) * ray::MISS_LANES + lane.x;
    let count = seen_at(seen, observed::MARKS_COUNT);
    if k >= count || k >= observed::MARKS {
        return;
    }
    let listed = seen_at(seen, observed::MARKS_AT + k);
    let id = UVec3::new(listed & 0xffff, listed >> 16, 0);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let w = missed(
        eye, window, ground, looks, shards, overwrites, pool, beams, seen, id, size,
    );
    write_ray(rays, id, w);
}
