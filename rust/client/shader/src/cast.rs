// pxl-shader/cast.rs

//! Each drawn pixel's ray (`cast_cs`) off the cell the raster kept at it
//! or the sky, and the rest walked (`miss_list_cs` over their list,
//! `miss_cs` over every pixel where the list was full), what lies about
//! the faces they stopped on asked once a stop, as a box's is
//! (`stop_lead_cs`, raster.rs `face_cs`), and put at their pixels
//! (`stop_faces_cs`).

use pxl_spec_core::cast::{self, End, Trace};
use pxl_spec_core::{beam, observed, raster, ray, tally, way};
use spirv_std::glam::{IVec4, UVec3, UVec4};

use spirv_std::{spirv, Image};

use crate::beam::{node_put, Beams, Words};
use crate::eye::{grid, Eye};
use crate::observed::{seen_at, seen_put, Seen};
use crate::observer::{self, Observer, Take};
use crate::world::{face_of, unasked, World};
use crate::{add, most, opens};

/// The eye's rays, a drawn pixel each, a layer a word of the ray's (`word`;
/// pxl-spec-core `ray`): the cast writes them.
pub(crate) type Rays = Image!(2D, format = r32ui, sampled = false, arrayed = true);

/// The visible sample at each drawn pixel (pxl-render gpu/ray/raster.rs): its
/// sample's number (pxl-spec-core `raster::id_of`), 0 where it kept none,
/// read as an array of one layer.
type Visibility = Image!(2D, type = u32, sampled = true, arrayed = true);

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four palette indices
/// the ground's asset is drawn by, a byte each (`ground::word`), or in the
/// steps view the nodes its descent asked about, then what lies about its
/// face (`World::faced`); where it met nothing, the first two zero.
fn word<A: Copy>(
    ray: &cast::Ray,
    met: Trace<A>,
    steps: bool,
    faces: u32,
) -> [u32; ray::WORDS as usize] {
    if met.ends != End::Hit {
        return [0, 0, met.visits, 0];
    }
    let s = met.hit;
    let [val, four] = s.asset;
    [
        ray::first_word(val, face_of(ray, s.axis), s.node.zoom),
        s.t.to_bits(),
        if steps { met.visits } else { four },
        faces,
    ]
}
const _: () = assert!(
    ray::FACES == 3,
    "what lies about a ray's face is its last word"
);

/// The tangents across and up of where drawn pixel `p`'s ray goes through it,
/// its middle off by the eye's jitter (pxl-spec-core `eye::JITTER`), of the
/// `size` the rays are, row 0 at the top, as the screen pass forms its own
/// pixel's: the screen's half-angles' scaled by where it lies.
pub(crate) fn px_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5 + eye.jitter.x) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5 + eye.jitter.y) / size.y as f32 * 2.0;
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

/// The tiles as a pass draws them (pxl-spec-core `screen::TILES_WORDS`): the
/// tiles across and down, the frames a tile stays claimed once changed and
/// this frame's number, then the world's drawn pixels and a quad's growth, which the cast
/// reads none of.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tiles {
    count: UVec4,
    size: spirv_std::glam::Vec4,
}

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
/// off the count in its dispatch's words (`marks`, `ray::MISS_ARGS_CNT`),
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
    #[spirv(descriptor_set = 0, binding = 13)] visibility: &Visibility,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] marks: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 30)] changed_at: &[u32],
    #[spirv(uniform, descriptor_set = 0, binding = 31)] tiles: &Tiles,
) {
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    // A tile not claimed is passed by, as the light's cast passes one by
    // (light.rs `Map::needs`): a frame not drawn whole casts the claimed
    // tiles' pixels alone (pxl-spec-core `screen`).
    let side = pxl_spec_core::screen::TILE;
    let tile = (id.y / side) * tiles.count.x + id.x / side;
    let age = pxl_spec_core::screen::age(tiles.count.w, changed_at[tile as usize]);
    if !pxl_spec_core::screen::claimed(age, tiles.count.z) {
        return;
    }
    let g = grid(eye);
    let px = px_tan(eye, id, size);
    let aim = eye.aim(px);
    let ray = aim.ray;
    // [naga] Fetched at u32s, as the raster wrote them.
    let kept: UVec4 = visibility.fetch(UVec3::new(id.x, id.y, 0));
    // The cell off its visible sample, the sky, or a walk of its own
    // (observer.rs `resolve`), the nearest cell entered at the reach where none
    // is.
    let (took, hit, k) = observer::resolve(eye, beams, px, &aim, kept.x);
    let steps = eye.origin.w != 0;
    let on = seen_at(seen, observed::TALLY_ON) != 0;
    // No sample about it met anything: the sky, taken without a walk of its
    // own, as a thing thinner than the samples' spacing between them goes
    // unseen.
    if took == Take::Sky {
        write_ray(
            rays,
            id,
            word(
                &ray,
                Trace {
                    ends: End::Miss,
                    hit,
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
    if took == Take::Sample {
        // What lies about its face, the sample's, and the nodes the
        // sample's walk visited, which the steps view shows as this pixel's.
        let (faces, visits) = (
            seen_at(beams, k + beam::SAMPLE_FACES),
            seen_at(beams, k + beam::SAMPLE_VISITS),
        );
        add_cnt(marks, on, tally::TOOK, 1);
        write_ray(
            rays,
            id,
            word(
                &ray,
                Trace {
                    ends: End::Hit,
                    hit,
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
    // takes its visible sample pays none of a walk's footprint.
    add_cnt(marks, on, tally::WALKED, 1);
    // SAFETY: within the rays, bounded by their size above.
    unsafe { rays.write(UVec3::new(id.x, id.y, 0), ray::MISS) };
    let whole = [size.x.div_ceil(CAST_SIDE), size.y.div_ceil(CAST_SIDE)];
    let at = enlist(marks, observed::MARKS, whole);
    if at < observed::MARKS {
        seen_put(seen, observed::MARKS_AT + at, id.y << 16 | id.x);
    }
}

/// `v` added to lane `lane` of a pixel's or a slot's (pxl-spec-core
/// `tally::TOOK` on) in its walk again's args `args`, where `on`
/// (`ray::MISS_ARGS_TALLY`), the eye's pixels and the light's slots alike.
pub(crate) fn add_cnt(args: &mut [u32], on: bool, lane: u32, v: u32) {
    if on {
        add(args, ray::MISS_ARGS_TALLY + lane - tally::TOOK, v);
    }
}

/// One more missed ray listed for its walk again, the eye's pixels and the
/// light's slots alike (pxl-shader light.rs `light_cs`): its place off the count
/// in its dispatch's words `args` (pxl-spec-core `ray::MISS_ARGS_CNT`),
/// the rows the list fills raised to hold it by the place that opens each
/// (`ray::MISS_ARGS_LIST`, `opens`); the first past the list's `room` has
/// the walk over `whole` workgroups dispatched instead
/// (`ray::MISS_ARGS_WHOLE`). The count starts each frame at 0
/// (`ray::MISS_ARGS_EMPTY`). Its place, which the caller writes where it is
/// under `room`.
pub(crate) fn enlist(args: &mut [u32], room: u32, whole: [u32; 2]) -> u32 {
    let at = add(args, ray::MISS_ARGS_CNT, 1);
    if at < room {
        opens(
            args,
            ray::MISS_ARGS_LIST + 1,
            at,
            ray::MISS_LANES * ray::MISS_ACROSS,
        );
    } else if at == room {
        most(args, ray::MISS_ARGS_WHOLE, whole[0]);
        most(args, ray::MISS_ARGS_WHOLE + 1, whole[1]);
    }
    at
}

/// Pixel `id`'s ray again from its cell's start, writing what it asks into
/// the observed pyramid (`Writing`): its words, what lies about the face it
/// stopped on told at once where it needs no asking (`world::unasked`),
/// else its stop kept for that to be asked once a stop (`stop`).
#[allow(clippy::too_many_arguments)]
fn missed(
    eye: &Eye,
    window: &[IVec4],
    ground: &[u32],
    palette_indices: &[u32],
    shards: &[IVec4],
    overwrites: &[u32],
    pool: &[u32],
    beams: &mut Beams,
    seen: &mut Seen,
    id: UVec3,
    size: UVec3,
) -> [u32; ray::WORDS as usize] {
    let world = World {
        window,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let (aim, met, before) =
        observer::walked(eye, world, seen, &*beams, px_tan(eye, id, size), &mut (), &mut ());
    let (asks, faces) = unasked(met.ends, met.hit.asset[0]);
    let faces = if asks {
        let face = face_of(&aim.ray, met.hit.axis);
        stop(eye, beams, seen, &met.hit.node, face, id)
    } else {
        faces
    };
    let met = Trace {
        visits: met.visits + before,
        ..met
    };
    word(&aim.ray, met, eye.origin.w != 0, faces)
}

/// The stop at `node` of pixel `id`'s walk again, entered through face
/// `face`, kept for what lies about that face to be asked once a stop
/// (pxl-spec-core `beam::STOP_WORDS`): numbered as the walks take them
/// (`observed::STOPS_CNT`), the dispatch over them raised by the number
/// that opens a workgroup (`observed::STOPS_ARGS`), and its number marked
/// waiting (`beam::STOP_WAITS`) as its pixel's last word until its faces
/// are put there (`stop_faces_cs`); none past `beam::STOPS`.
fn stop(
    eye: &Eye,
    beams: &mut Beams,
    seen: &mut Seen,
    node: &cast::Node,
    face: u32,
    id: UVec3,
) -> u32 {
    let r = seen.add(observed::STOPS_CNT, 1);
    if r >= beam::STOPS {
        return 0;
    }
    opens(seen, observed::STOPS_ARGS, r, beam::STOP_LANES);
    let k = grid(eye).stop_at(r);
    node_put(beams, k + beam::STOP_NODE, node);
    beams.set(k + beam::STOP_FACE, face);
    beams.set(k + beam::STOP_PX, id.y << 16 | id.x);
    beam::STOP_WAITS | r
}
const _: () = assert!(
    pxl_spec_core::face::BITS < 31,
    "a waiting stop's mark lies past every face's bit"
);

/// Whether the stops at words `a` and `b` are one: one node entered
/// through one face, so what lies about it is one.
fn alike(w: &Beams, a: u32, b: u32) -> bool {
    let mut k = 0;
    while k < beam::STOP_PX {
        if w.word(a + k) != w.word(b + k) {
            return false;
        }
        k += 1;
    }
    true
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
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let w = missed(
        eye, window, ground, palette_indices, shards, overwrites, pool, beams, seen, id, size,
    );
    write_ray(rays, id, w);
}

/// Each pixel `cast_cs` listed (pxl-spec-core `observed::MARKS_AT`), its
/// ray again (`missed`): a thread a listed pixel, `ray::MISS_LANES` a
/// workgroup and `ray::MISS_ACROSS` workgroups a row, dispatched over the
/// rows the list filled (`ray::MISS_ARGS_LIST`), each past the count
/// returning at once. The count is copied into the pyramid's buffer once
/// `cast_cs` is done (`observed::MARKS_CNT`).
#[spirv(compute(threads(64, 1, 1), entry_point_name = "miss_list_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn miss_list_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] window: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &mut Seen,
) {
    let window: &[IVec4] = window;
    let k = (group.y * ray::MISS_ACROSS + group.x) * ray::MISS_LANES + lane.x;
    let cnt = seen_at(seen, observed::MARKS_CNT);
    if k >= cnt || k >= observed::MARKS {
        return;
    }
    let listed = seen_at(seen, observed::MARKS_AT + k);
    let id = UVec3::new(listed & 0xffff, listed >> 16, 0);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let w = missed(
        eye, window, ground, palette_indices, shards, overwrites, pool, beams, seen, id, size,
    );
    write_ray(rays, id, w);
}

/// The missed pixels' stops the walks again kept (`stop`), as many as they
/// took, `beam::STOPS` at most.
fn stops(seen: &Seen) -> u32 {
    seen_at(seen, observed::STOPS_CNT).min(beam::STOPS)
}

/// The number of the stop pixel (`x`, `y`) waits for, where it is one
/// alike the stop at words `k` (`alike`); `beam::STOPS` where not.
fn waits_alike(rays: &Rays, beams: &Beams, g: &beam::Grid, k: u32, x: u32, y: u32) -> u32 {
    let w: u32 = rays.read(UVec3::new(x, y, ray::FACES));
    if w & beam::STOP_WAITS == 0 {
        return beam::STOPS;
    }
    let r = w & !beam::STOP_WAITS;
    if r >= beam::STOPS || !alike(beams, g.stop_at(r), k) {
        return beam::STOPS;
    }
    r
}

/// Each stop the walks again kept (`stop`), a thread each,
/// `beam::STOP_LANES` a workgroup, dispatched off their count
/// (`observed::STOPS_ARGS`): the one that asks what lies about its face is
/// one whose pixel's neighbors to the left and above wait for no stop alike
/// (`alike`) within its square of `beam::STOP_RUN` a side, so a patch of
/// stops alike is asked once a square, not once a pixel or a row, and
/// which of them asks is a function of the pixels alone. It is listed with
/// its ask (`beam::Grid::stop_ask_at`) for the faces' pass as a box is
/// (raster.rs `face_cs`; `ray::MISS_ARGS_ASKED` of the walk again's words,
/// its count, and its dispatch raised by the place that opens a
/// workgroup), off the line its cell's rays resumed from. Any other is told
/// the neighbor's stop it takes them from, the left before the one above
/// (`beam::STOP_FOLLOWS`), which `stop_faces_cs` follows to the one that
/// asks. It reads the stops and its neighbors' last ray words, which the
/// walks wrote, and writes only its own stop's last word and its ask, so
/// no thread reads what another writes; the pyramid's head, which it is
/// dispatched off, it only reads.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "stop_lead_cs"))]
pub fn stop_lead_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] marks: &mut [u32],
) {
    if id.x >= stops(seen) {
        return;
    }
    let g = grid(eye);
    let k = g.stop_at(id.x);
    let px = beams.word(k + beam::STOP_PX);
    let (x, y) = (px & 0xffff, px >> 16);
    // The neighbor it takes its faces from: the left, else the one above,
    // each where its pixel waits for a stop alike within the square.
    let mut from = beam::STOPS;
    if x % beam::STOP_RUN != 0 {
        from = waits_alike(rays, beams, &g, k, x - 1, y);
    }
    if from == beam::STOPS && y % beam::STOP_RUN != 0 {
        from = waits_alike(rays, beams, &g, k, x, y - 1);
    }
    if from != beam::STOPS {
        beams.set(k + beam::STOP_OUT, beam::STOP_FOLLOWS | from);
        return;
    }
    let asked = ray::MISS_ARGS_ASKED;
    let place = add(marks, asked + raster::ARGS_INSTANCES, 1);
    opens(
        marks,
        asked + raster::ARGS_FACES,
        place,
        raster::FACES_LANES,
    );
    let size: UVec3 = rays.query_size();
    let cell = g.cell(px_tan(eye, UVec3::new(x, y, 0), size));
    let corner = beams.word(g.cell_at(cell) + beam::CELL_CORNER);
    let face = beams.word(k + beam::STOP_FACE);
    let a = g.stop_ask_at(place);
    beams.set(a + beam::ASK_NODE, k + beam::STOP_NODE);
    beams.set(a + beam::ASK_FACE, face);
    beams.set(a + beam::ASK_LINE, beam::Grid::line(corner));
    beams.set(a + beam::ASK_TO, k + beam::STOP_OUT);
}

/// Each stop the walks again kept, a thread each as `stop_lead_cs`, once
/// the faces' pass asked what lies about each patch's face (raster.rs
/// `face_cs`): those faces put at its pixel as its last ray word
/// (`ray::FACES`), its own where it asked them, else those of the stop that
/// asked for it, reached through the neighbors each was told
/// (`beam::STOP_FOLLOWS`), a step left or up within the square each, so
/// two of its sides at most; no pixel is left waiting past the cast's
/// pass.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "stop_faces_cs"))]
pub fn stop_faces_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] seen: &Seen,
) {
    if id.x >= stops(seen) {
        return;
    }
    let g = grid(eye);
    let k = g.stop_at(id.x);
    let px = beams.word(k + beam::STOP_PX);
    let mut faces = beams.word(k + beam::STOP_OUT);
    let mut steps = 0;
    while faces & beam::STOP_FOLLOWS != 0 && steps < 2 * beam::STOP_RUN {
        faces = beams.word(g.stop_at(faces & !beam::STOP_FOLLOWS) + beam::STOP_OUT);
        steps += 1;
    }
    // SAFETY: a pixel the walk again wrote, within the rays.
    unsafe { rays.write(UVec3::new(px & 0xffff, px >> 16, ray::FACES), faces) };
}
