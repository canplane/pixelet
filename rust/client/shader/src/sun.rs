// pxl-shader/sun.rs

//! The world's depth from the sun into its clipmap (pxl-spec-core `sun`),
//! every slot each frame it is cast, as the eye's rays are: the beam rays
//! and their cells' starts (`sun_beam_cs`, `sun_cells_cs`), the samples
//! (`sun_sample_cs`) and their boxes (`sun_boxes_cs`), which the raster
//! draws, every slot taking its pick (`sun_cs`) and the rest walked
//! (`sun_miss_cs`).

use pxl_spec_core::cast::{self, Detail, Until};
use pxl_spec_core::fill::put;
use pxl_spec_core::{at, beam, observed, raster, ways};
use spirv_std::glam::{IVec4, UVec3, UVec4, Vec4};
use spirv_std::{spirv, Image};

use crate::add;
use crate::beam::{resumed, sample_put, Beamed, Leaving};
use crate::cast::listed;
use crate::eye::{eye_origin, Eye};
use crate::observed::{seen_at, Seen, Writing};
use crate::raster::{boxed, picked, NONE};
use crate::world::{Unlooked, World};

/// The world's depth from the sun, a slot of the sun's clipmap each
/// (pxl-spec-core `sun`): the slot's texel of the light box, and its ray
/// from the box's near plane along the sun's rays, its block taken off the
/// raster's pick or walked through the world's one query to the block that
/// fills (`cast::first`) as the eye's are, and the word it leaves there,
/// every frame the map is cast. Its one layer read and written in place,
/// the one format family WebGPU lets a shader do both to, as the rays' are.
type SunWords = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The raster's pick at each texel of the box, in the
/// box's order (pxl-spec-core `sun::Sun::texel`): its sample's number
/// (`raster::id_of`), 0 where it kept none, read as an array of one layer,
/// as the eye's pixels read theirs.
type Picks = Image!(2D, type = u32, sampled = true, arrayed = true);

/// What the sun's cast is handed beside the eye's words (pxl-spec-core
/// `sun::MOVED_HEAD`): whether it is counted (`sun::COUNTS`), and where the
/// rays start (`sun::Starts`).
type Moved = [IVec4];

/// Where the rays start (`sun::Starts`), off the head.
fn starts(m: &Moved) -> pxl_spec_core::sun::Starts {
    pxl_spec_core::sun::Starts::of_words(|k| {
        let v = m[pxl_spec_core::sun::STARTS_AT + k];
        [v.x, v.y, v.z, v.w]
    })
}

fn counted(m: &Moved) -> bool {
    m[0].y != 0
}

/// The sun's cast off the eye's words (pxl-spec-core `sun::Sun::words`).
fn sun_of(eye: &Eye) -> pxl_spec_core::sun::Sun {
    let v3 = |v: Vec4| [v.x, v.y, v.z];
    let i = |v: Vec4| v.w.to_bits() as i32;
    pxl_spec_core::sun::Sun {
        first: v3(eye.sun[0]),
        reach: eye.sun[0].w,
        across: v3(eye.sun[1]),
        down: v3(eye.sun[2]),
        dir: v3(eye.sun[3]),
        bias: v3(eye.sun[4]),
        corner: [i(eye.sun[1]), i(eye.sun[2])],
    }
}

/// Slot `slot` of the clipmap `size` a side: its box texel, and its ray,
/// from where and at what zooms (`sun::ray`, `Sun::detail`).
fn sun_slot(
    eye: &Eye,
    moved: &Moved,
    size: u32,
    slot: UVec3,
) -> ([u32; 2], [i32; 3], cast::Ray, Detail) {
    let s = sun_of(eye);
    let p = s.texel(size, [slot.x, slot.y]);
    let (origin, detail) = eye_origin(eye);
    let mut detail = s.detail(detail);
    let (o, ray, from) = pxl_spec_core::sun::ray(origin, &s, &starts(moved), p);
    detail.eye = from;
    (p, o, ray, detail)
}

const _: () = assert!(
    pxl_spec_core::sun::TILE == 8,
    "`sun_cs` is a workgroup a tile, its threads a side of it"
);

/// The world's depth from the sun into every slot: a workgroup a tile
/// (`sun::TILE`), a thread a slot, each taking the raster's pick
/// where its own ray enters it (`picked`), as the eye's pixels take theirs
/// (pxl-shader cast.rs `cast_cs`), and the word it leaves there; the rest
/// listed for their walk (`sun_miss_cs`), every slot where the raster is not
/// made yet.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cs"))]
pub fn sun_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] counts: &mut [u32],
    #[spirv(descriptor_set = 0, binding = 15)] picks: &Picks,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 17)] misses: &mut [u32],
) {
    let t = pxl_spec_core::sun::TILE;
    let size: UVec3 = sun.query_size();
    let slot = UVec3::new(group.x * t + lane.x, group.y * t + lane.y, 0);
    if slot.x >= size.x || slot.y >= size.y {
        return;
    }
    let (p, o, ray, detail) = sun_slot(eye, moved, size.x, slot);
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = picks.fetch(UVec3::new(p[0], p[1], 0));
    let pick = kept.x;
    if pick != 0 && pick & raster::LACK == 0 {
        let k = pxl_spec_core::sun::sample(size.x, raster::sample_of(pick));
        let (stop, enters) = picked(&*list, k, o, &ray);
        if enters {
            let met = cast::Met {
                ends: cast::Ends::Stop,
                stop,
                visits: 0,
            };
            if counted(moved) {
                add(counts, pxl_spec_core::sun::COUNT_PICKED, 1);
            }
            let s = sun_of(eye);
            // SAFETY: within the map, bounded by its size above.
            unsafe { sun.write(slot, s.word(detail, pxl_spec_core::sun::depth(&s, &met))) };
            return;
        }
    }
    // Listed for its walk again as the eye's pixels are (cast.rs `listed`):
    // the list holds every slot, so none finds it full.
    let at = listed(misses, size.x * size.y, [0; 2]);
    put(
        list,
        pxl_spec_core::sun::misses_at(size.x) + at,
        slot.y << 16 | slot.x,
    );
}

const _: () = assert!(
    pxl_spec_core::ray::MISS_LANES == 64,
    "`sun_miss_cs` is a workgroup of `ray::MISS_LANES` slots, as `miss_list_cs` is"
);

/// The world's depth from the sun into each slot `sun_cs` listed: a thread
/// a slot, `ray::MISS_LANES` a workgroup and `ray::MISS_ACROSS` workgroups
/// a row, each past the count returning at once, as the eye's listed pixels
/// are (cast.rs `miss_list_cs`), its ray
/// walked off the beam to the block that fills, and the word it leaves
/// there.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "sun_miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_miss_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = sun.query_size();
    let i = (group.y * pxl_spec_core::ray::MISS_ACROSS + group.x) * pxl_spec_core::ray::MISS_LANES
        + lane.x;
    if i >= at(list, pxl_spec_core::sun::misses_count(size.x)) {
        return;
    }
    let w = at(list, pxl_spec_core::sun::misses_at(size.x) + i);
    let slot = UVec3::new(w & 0xffff, w >> 16, 0);
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let (p, o, ray, detail) = sun_slot(eye, moved, size.x, slot);
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_MISSED_PASS);
    let met = sun_resumed(&mut Writing {
            world,
            seen,
            tag,
            blocks: false,
        }, list, size.x, p, o, &ray, detail);
    let s = sun_of(eye);
    // SAFETY: a slot `sun_cs` listed, within the map.
    unsafe { sun.write(slot, s.word(detail, pxl_spec_core::sun::depth(&s, &met))) };
}

/// Each sample of the sun's map (pxl-spec-core `sun::SAMPLE`), a slot every
/// `SAMPLE` across and down, a thread each: its ray walked off the beam as
/// a slot's is (`sun_miss_cs`), and its stop left in the eye's samples' words (`sample_put`) for
/// the raster (`sun_boxes_cs`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_sample_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_sample_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = sun.query_size();
    let d = pxl_spec_core::sun::SAMPLE;
    if id.x >= size.x / d || id.y >= size.x / d {
        return;
    }
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let s = sun_of(eye);
    let p = s.texel(size.x, [id.x * d, id.y * d]);
    let (origin, detail) = eye_origin(eye);
    let mut detail = s.detail(detail);
    let (o, ray, from) = pxl_spec_core::sun::ray(origin, &s, &starts(moved), p);
    detail.eye = from;
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_SAMPLES_PASS);
    let met = sun_resumed(&mut Writing {
            world,
            seen,
            tag,
            blocks: false,
        }, list, size.x, p, o, &ray, detail);
    let index = id.y * (size.x / d) + id.x;
    sample_put(list, pxl_spec_core::sun::sample(size.x, index), &met);
}

/// Each sample of the sun's map (`sun_sample_cs`), a
/// thread each, written as a box for the raster's draw by the eye's writer
/// (raster.rs `boxed`), the one across and up before it, from where box
/// texel (0, 0)'s ray starts; the sun's has no box about its eye (pxl-spec-core
/// `sun::SAMPLE`).
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_boxes_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_boxes_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 13)] counts: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let size: UVec3 = sun.query_size();
    let d = pxl_spec_core::sun::SAMPLE;
    // [naga] The eye's vec2 of u32s read, so the module holds the type
    // naga's SPIR-V reader splits an arrayed image's layer off by (`Eye`).
    if id.x >= size.x / d || id.y >= size.x / d || eye.range.x > eye.range.y {
        return;
    }
    let n = size.x / d;
    let at = |i: u32, j: u32| pxl_spec_core::sun::sample(size.x, j * n + i);
    let before = [
        if id.x > 0 {
            at(id.x - 1, id.y)
        } else {
            NONE
        },
        if id.y > 0 {
            at(id.x, id.y - 1)
        } else {
            NONE
        },
    ];
    // From where box texel (0, 0)'s ray starts, as the raster lays the
    // texels' rays off it (pxl-render raster.wgsl), in the frame the slots'
    // rays start in (`sun::Starts`).
    let s = sun_of(eye);
    let origin = starts(moved).start(s.corner);
    let box_at = boxed(
        list,
        at(id.x, id.y),
        before,
        origin,
        id.y * n + id.x,
        (false, [0.0; 2]),
        boxes,
        args,
    );
    if box_at != NONE && counted(moved) {
        add(counts, pxl_spec_core::sun::COUNT_BOXES, 1);
    }
}

/// Each beam ray of the sun's map (pxl-spec-core `sun::BEAM`), a thread
/// each, `sun::beam_across` a side, `sun::BEAM_WINDOW` of them past the box
/// on each side: its ray from where its texel of the box lies on the near
/// plane (`sun::Starts`), walked from the world's root to the first block
/// that fills or lacks or the first split narrower than `2√2` of the
/// beam's spacing (`cast::Until::Beam`, `cast::Detail::beam`), its line
/// left for the texels about it (`SunLeaving`) and how far they may start
/// from it, its stop less `sun::margin`.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_beam_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn sun_beam_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 11)] moved: &Moved,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = sun.query_size();
    let across = pxl_spec_core::sun::beam_across(size.x);
    if id.x >= across || id.y >= across {
        return;
    }
    let world = World {
        window,
        ground,
        looks: Unlooked(0),
        shards,
        overwrites,
        eye,
        pool: ways::tree::Read(pool),
    };
    let s = sun_of(eye);
    let (b, w) = (
        pxl_spec_core::sun::BEAM as i32,
        pxl_spec_core::sun::BEAM_WINDOW as i32,
    );
    // Its texel of the box, which may lie past it.
    let p = [(id.x as i32 - w) * b, (id.y as i32 - w) * b];
    let o = starts(moved).start([
        s.corner[0].wrapping_add(p[0]),
        s.corner[1].wrapping_add(p[1]),
    ]);
    let (origin, detail) = eye_origin(eye);
    let mut detail = s.detail(detail);
    let q3 = (1u32 << pxl_spec_core::FRAC_W) as f32;
    detail.eye = [
        origin[0].wrapping_sub(o[0]) as f32 / q3,
        origin[1].wrapping_sub(o[1]) as f32 / q3,
        origin[2].wrapping_sub(o[2]) as f32 / q3,
    ];
    let width = s.beam_width();
    detail.beam = width;
    let ray = cast::Ray::new(s.dir, s.reach);
    let base = pxl_spec_core::sun::line(size.x, id.y * across + id.x);
    let k = base + pxl_spec_core::sun::LINE_ORIGIN;
    put(list, k, o[0] as u32);
    put(list, k + 1, o[1] as u32);
    put(list, k + 2, o[2] as u32);
    let tag = observed::tag(seen_at(seen, observed::FRAME), observed::SUN_BEAM_PASS);
    let met = cast::rooted(
        &mut Writing {
            world,
            seen,
            tag,
            blocks: false,
        },
        o,
        &ray,
        detail,
        Until::Beam,
        &mut Leaving(list, base),
    );
    let t = met.stop.t - pxl_spec_core::sun::margin(width);
    put(
        list,
        base + beam::LINE_T,
        if t > 0.0 { t } else { 0.0 }.to_bits(),
    );
}

/// Where each cell of the sun's map starts (pxl-spec-core `sun::cell`), a
/// thread a cell, `sun::BEAM` texels a side: the least of the beam rays
/// within `sun::BEAM_WINDOW` of its corners, and that ray, whose line its
/// texels resume from.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sun_cells_cs"))]
pub fn sun_cells_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 10)] sun: &SunWords,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 12)] list: &mut [u32],
) {
    let size: UVec3 = sun.query_size();
    let cells = pxl_spec_core::sun::cells_across(size.x);
    // [naga] The eye's vec2 of u32s read, so the module holds the type
    // naga's SPIR-V reader splits an arrayed image's layer off by (`Eye`).
    if id.x >= cells || id.y >= cells || eye.range.x > eye.range.y {
        return;
    }
    let across = pxl_spec_core::sun::beam_across(size.x);
    let side = 2 + 2 * pxl_spec_core::sun::BEAM_WINDOW;
    // Its own corners lie `BEAM_WINDOW` in from the rays' first.
    let first = id.y * across + id.x;
    let mut from = first;
    let mut least = f32::from_bits(at(
        list,
        pxl_spec_core::sun::line(size.x, first) + beam::LINE_T,
    ));
    let mut j = 0;
    while j < side {
        let mut i = 0;
        while i < side {
            let r = (id.y + j) * across + id.x + i;
            let t = f32::from_bits(at(
                list,
                pxl_spec_core::sun::line(size.x, r) + beam::LINE_T,
            ));
            if t < least {
                least = t;
                from = r;
            }
            i += 1;
        }
        j += 1;
    }
    let c = pxl_spec_core::sun::cell(size.x, [id.x, id.y]);
    put(list, c + pxl_spec_core::sun::CELL_T, least.to_bits());
    put(list, c + pxl_spec_core::sun::CELL_RAY, from);
}

/// The walk of a ray from `o` of box texel `p` of the map `size` a side,
/// resumed off the beam (pxl-spec-core `sun::BEAM`) as the eye's pixels
/// resume (pxl-shader beam.rs `resumed`): from its cell's start
/// (`sun::cell`), in the deepest node of that start's beam ray's line holding
/// where it is there, moved to its own origin (`cast::Node::moved`), as
/// every sun ray starts apart, or the root where none in that root does;
/// through `q`, the observed pyramid as a writing walk reads it.
fn sun_resumed<'b, Q: cast::Query>(
    q: &mut Q,
    list: &'b [u32],
    size: u32,
    p: [u32; 2],
    o: [i32; 3],
    ray: &cast::Ray,
    detail: Detail,
) -> cast::Met<Q::At>
where
    Beamed<'b, [u32]>: cast::Over<Q::At>,
{
    let b = pxl_spec_core::sun::BEAM;
    let c = pxl_spec_core::sun::cell(size, [p[0] / b, p[1] / b]);
    let t = f32::from_bits(at(list, c + pxl_spec_core::sun::CELL_T));
    let base = pxl_spec_core::sun::line(size, at(list, c + pxl_spec_core::sun::CELL_RAY));
    let line = Beamed(list, base);
    let k = base + pxl_spec_core::sun::LINE_ORIGIN;
    let from = [
        at(list, k) as i32,
        at(list, k + 1) as i32,
        at(list, k + 2) as i32,
    ];
    resumed(q, o, ray, detail, t, &line, line.node().moved(from, o))
}
