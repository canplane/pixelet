// pxl-shader/cast.rs

//! Each drawn pixel's ray (`cast_cs`) off the cell the raster kept at it
//! or the sky, and the rest walked (`miss_list_cs` over their list,
//! `miss_cs` over every pixel where the list was full), what lies about
//! the faces they stopped on asked once a hit, as a box's is
//! (`hit_lead_cs`, raster.rs `face_cs`), and put at their pixels
//! (`hit_faces_cs`).

use pxl_engine::cast::{self, End, Trace};
use pxl_engine::{beam, pyramid, raster, ray, tally, way};
use pxl_shader_macro::compute;
use spirv_std::glam::{IVec4, UVec3, UVec4};

use spirv_std::{spirv, Image};

use super::Rays;
use crate::beam::{node_put, Beams, Words};
use crate::eye::{grid, Eye};
use crate::pyramid::{pyramid_at, pyramid_put, Pyramid};
use crate::observer::{self, Observer, Take};
use crate::workgroup::CAST_SIDE;
use crate::world::{face_of, unasked_face, World};
use crate::{add, most, opens};

/// The visible sample at each drawn pixel (pxl-render gpu/ray/raster.rs): its
/// sample's number (pxl-engine `raster::id_of`), 0 where it kept none,
/// read as an array of one layer.
type Visibility = Image!(2D, type = u32, sampled = true, arrayed = true);

/// A ray's words, a layer each of `Rays`: the value it stopped on and the
/// face it entered through, then how far as f32 bits, then the four palette indices
/// the ground's asset is drawn by, a byte each (`ground::word`), or in the
/// steps view the nodes its descent asked about, then what lies about its
/// face (`World::ask_face`); where it met nothing, the first two zero.
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
/// its middle off by the eye's jitter (pxl-engine `eye::JITTER`), of the
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

/// The tiles as a pass draws them (pxl-engine `screen::TILES_WORDS`): the
/// tiles across and down, the frames a tile stays claimed once changed and
/// this frame's number, then the world's drawn pixels and a quad's growth, which the cast
/// reads none of.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tiles {
    count: UVec4,
    size: spirv_std::glam::Vec4,
}

/// Each pixel's ray: the cell the raster kept at it (pxl-engine
/// `raster`; pxl-render gpu/ray/raster.rs), the nearest a sample stopped at
/// that its ray enters across the axis that sample entered by, tested again
/// exactly (`cast::entry`) where a GPU's f32 may part the two, its words, no
/// step its own, with what lies about its face, its sample's; the sky
/// where no sample about it met anything. Any other pixel, where it kept a
/// node a sample lacks at, or none, or the test refuses it, takes no walk
/// here: its first word is the mark the walk again goes by (pxl-engine
/// `ray::MISS`; `miss_list_cs`, `miss_cs`), so no lane carries a walk's
/// footprint for the few that walk. A marked pixel is listed for that walk
/// (pxl-engine `pyramid::MARKS_AT`) by a plain write, its place taken
/// off the count in its dispatch's words (`marks`, `ray::MISS_ARGS_CNT`),
/// so no word of the pyramid is typed atomic here; one past the list's room
/// has the walk over every pixel dispatched instead.
#[compute(threads(CAST_SIDE, CAST_SIDE), entry_point_name = "cast_cs")]
#[allow(clippy::too_many_arguments)]
pub fn cast_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
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
    // tiles' pixels alone (pxl-engine `screen`).
    let side = pxl_engine::screen::TILE;
    let tile = (id.y / side) * tiles.count.x + id.x / side;
    let age = pxl_engine::screen::age(tiles.count.w, changed_at[tile as usize]);
    if !pxl_engine::screen::claimed(age, tiles.count.z) {
        return;
    }
    let g = grid(eye);
    let px = px_tan(eye, id, size);
    let aim = eye.aim(px);
    let ray = aim.ray;
    // [naga] Fetched at u32s, as the raster wrote them.
    let visible: UVec4 = visibility.fetch(UVec3::new(id.x, id.y, 0));
    // The cell off its visible sample, the sky, or a walk of its own
    // (observer.rs `resolve`), the nearest cell entered at the reach where none
    // is.
    let (took, hit, k) = observer::resolve(eye, beams, px, &aim, visible.x);
    let steps = eye.origin.w != 0;
    let on = pyramid_at(pyramid, pyramid::TALLY_ON) != 0;
    // No sample about it met anything: the sky, taken without a walk of its
    // own, as a thing thinner than the samples' spacing between them goes
    // unseen.
    if took == Take::Sky {
        add_cnt(marks, on, tally::SKY, 1);
        write_ray(
            rays,
            id,
            word(
                &ray,
                Trace {
                    ends: End::Miss,
                    hit,
                    visits: if steps {
                        pyramid_at(
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
            pyramid_at(beams, k + beam::SAMPLE_FACES),
            pyramid_at(beams, k + beam::SAMPLE_VISITS),
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
    let at = enlist(marks, pyramid::MARKS, whole);
    if at < pyramid::MARKS {
        pyramid_put(pyramid, pyramid::MARKS_AT + at, id.y << 16 | id.x);
    }
}

/// `v` added to lane `lane` of a pixel's or a slot's (pxl-engine
/// `tally::TOOK` on) in its walk again's args `args`, where `on`
/// (`ray::MISS_ARGS_TALLY`), the eye's pixels and the light's slots alike.
pub(crate) fn add_cnt(args: &mut [u32], on: bool, lane: u32, v: u32) {
    if on {
        add(args, ray::MISS_ARGS_TALLY + lane - tally::TOOK, v);
    }
}

/// One more missed ray listed for its walk again, the eye's pixels and the
/// light's slots alike (pxl-shader light.rs `light_cs`): its place off the count
/// in its dispatch's words `args` (pxl-engine `ray::MISS_ARGS_CNT`),
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
/// stopped on told at once where it needs no asking (`world::unasked_face`),
/// else its hit kept for that to be asked once a hit (`keep_hit`).
#[allow(clippy::too_many_arguments)]
fn walk_missed(
    eye: &Eye,
    clipmap: &[IVec4],
    ground: &[u32],
    palette_indices: &[u32],
    shards: &[IVec4],
    overwrites: &[u32],
    pool: &[u32],
    beams: &mut Beams,
    pyramid: &mut Pyramid,
    id: UVec3,
    size: UVec3,
) -> [u32; ray::WORDS as usize] {
    let world = World {
        clipmap,
        ground,
        palette_indices,
        shards,
        overwrites,
        eye,
        pool: way::tree::Read(pool),
    };
    let (aim, met, before) =
        observer::walk_again(eye, world, pyramid, &*beams, px_tan(eye, id, size), &mut ());
    let (asks, faces) = unasked_face(met.ends, met.hit.asset[0]);
    let faces = if asks {
        let face = face_of(&aim.ray, met.hit.axis);
        keep_hit(eye, beams, pyramid, &met.hit.node, face, id)
    } else {
        faces
    };
    let met = Trace {
        visits: met.visits + before,
        ..met
    };
    word(&aim.ray, met, eye.origin.w != 0, faces)
}

/// The hit at `node` of pixel `id`'s walk again, entered through face
/// `face`, kept for what lies about that face to be asked once a hit
/// (pxl-engine `beam::HIT_WORDS`): numbered as the walks take them
/// (`pyramid::HITS_CNT`), the dispatch over them raised by the number
/// that opens a workgroup (`pyramid::HITS_ARGS`), and its number marked
/// waiting (`beam::HIT_WAITS`) as its pixel's last word until its faces
/// are put there (`hit_faces_cs`); none past `beam::HITS`.
fn keep_hit(
    eye: &Eye,
    beams: &mut Beams,
    pyramid: &mut Pyramid,
    node: &cast::Node,
    face: u32,
    id: UVec3,
) -> u32 {
    let r = pyramid.add(pyramid::HITS_CNT, 1);
    if r >= beam::HITS {
        return 0;
    }
    opens(pyramid, pyramid::HITS_ARGS, r, beam::HIT_LANES);
    let k = grid(eye).hit_at(r);
    node_put(beams, k + beam::HIT_NODE, node);
    beams.set(k + beam::HIT_FACE, face);
    beams.set(k + beam::HIT_PX, id.y << 16 | id.x);
    beam::HIT_WAITS | r
}
const _: () = assert!(
    pxl_engine::face::BITS < 31,
    "a waiting hit's mark lies past every face's bit"
);

/// Whether the stops at words `a` and `b` are one: one node entered
/// through one face, so what lies about it is one.
fn alike(w: &Beams, a: u32, b: u32) -> bool {
    let mut k = 0;
    while k < beam::HIT_PX {
        if w.word(a + k) != w.word(b + k) {
            return false;
        }
        k += 1;
    }
    true
}

/// Each pixel `cast_cs` marked missed and could not list, its ray again
/// (`walk_missed`): a thread a drawn pixel, each but the marked returning at
/// once, dispatched only where the list was full (`ray::MISS_ARGS_WHOLE`),
/// after the list's walk, so a listed pixel it finds is no longer marked.
/// The two run after `cast_cs` in the cast's one pass and write every
/// marked pixel's words whole, so the mark never outlives the pass.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "miss_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn miss_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
) {
    let clipmap: &[IVec4] = clipmap;
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let first: u32 = rays.read(UVec3::new(id.x, id.y, 0));
    if first != ray::MISS {
        return;
    }
    let w = walk_missed(
        eye, clipmap, ground, palette_indices, shards, overwrites, pool, beams, pyramid, id, size,
    );
    write_ray(rays, id, w);
}

/// Each pixel `cast_cs` listed (pxl-engine `pyramid::MARKS_AT`), its
/// ray again (`walk_missed`): a thread a listed pixel, `ray::MISS_LANES` a
/// workgroup and `ray::MISS_ACROSS` workgroups a row, dispatched over the
/// rows the list filled (`ray::MISS_ARGS_LIST`), each past the count
/// returning at once. The count is copied into the pyramid's buffer once
/// `cast_cs` is done (`pyramid::MARKS_CNT`).
#[compute(threads(ray::MISS_LANES), entry_point_name = "miss_list_cs")]
#[allow(clippy::too_many_arguments)]
pub fn miss_list_cs(
    #[spirv(workgroup_id)] group: UVec3,
    #[spirv(local_invocation_id)] lane: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &mut [IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 4)] palette_indices: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &mut Pyramid,
) {
    let clipmap: &[IVec4] = clipmap;
    let k = (group.y * ray::MISS_ACROSS + group.x) * ray::MISS_LANES + lane.x;
    let cnt = pyramid_at(pyramid, pyramid::MARKS_CNT);
    if k >= cnt || k >= pyramid::MARKS {
        return;
    }
    let listed = pyramid_at(pyramid, pyramid::MARKS_AT + k);
    let id = UVec3::new(listed & 0xffff, listed >> 16, 0);
    let size: UVec3 = rays.query_size();
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    let w = walk_missed(
        eye, clipmap, ground, palette_indices, shards, overwrites, pool, beams, pyramid, id, size,
    );
    write_ray(rays, id, w);
}

/// The missed pixels' hits the walks again kept (`keep_hit`), as many as they
/// took, `beam::HITS` at most.
fn hits(pyramid: &Pyramid) -> u32 {
    pyramid_at(pyramid, pyramid::HITS_CNT).min(beam::HITS)
}

/// The number of the hit pixel (`x`, `y`) waits for, where it is one
/// alike the hit at words `k` (`alike`); `beam::HITS` where not.
fn alike_hit(rays: &Rays, beams: &Beams, g: &beam::Grid, k: u32, x: u32, y: u32) -> u32 {
    let w: u32 = rays.read(UVec3::new(x, y, ray::FACES));
    if w & beam::HIT_WAITS == 0 {
        return beam::HITS;
    }
    let r = w & !beam::HIT_WAITS;
    if r >= beam::HITS || !alike(beams, g.hit_at(r), k) {
        return beam::HITS;
    }
    r
}

/// Each hit the walks again kept (`keep_hit`), a thread each,
/// `beam::HIT_LANES` a workgroup, dispatched off their count
/// (`pyramid::HITS_ARGS`): the one that asks what lies about its face is
/// one whose pixel's neighbors to the left and above wait for no hit alike
/// (`alike`) within its square of `beam::HIT_SQUARE` a side, so a patch of
/// hits alike is asked once a square, not once a pixel or a row, and
/// which of them asks is a function of the pixels alone. It is listed with
/// its ask (`beam::Grid::hit_ask_at`) for the faces' pass as a box is
/// (raster.rs `face_cs`; `ray::MISS_ARGS_ROOM` of the walk again's words,
/// its count, and its dispatch raised by the place that opens a
/// workgroup), off the line its cell's rays resumed from. Any other is told
/// the neighbor's hit it takes them from, the left before the one above
/// (`beam::HIT_FOLLOWS`), which `hit_faces_cs` follows to the one that
/// asks. It reads the hits and its neighbors' last ray words, which the
/// walks wrote, and writes only its own hit's last word and its ask, so
/// no thread reads what another writes; the pyramid's head, which it is
/// dispatched off, it only reads.
#[compute(threads(beam::HIT_LANES), entry_point_name = "hit_lead_cs")]
pub fn hit_lead_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &Pyramid,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] marks: &mut [u32],
) {
    if id.x >= hits(pyramid) {
        return;
    }
    let g = grid(eye);
    let k = g.hit_at(id.x);
    let px = beams.word(k + beam::HIT_PX);
    let (x, y) = (px & 0xffff, px >> 16);
    // The neighbor it takes its faces from: the left, else the one above,
    // each where its pixel waits for a hit alike within the square.
    let mut from = beam::HITS;
    if x % beam::HIT_SQUARE != 0 {
        from = alike_hit(rays, beams, &g, k, x - 1, y);
    }
    if from == beam::HITS && y % beam::HIT_SQUARE != 0 {
        from = alike_hit(rays, beams, &g, k, x, y - 1);
    }
    if from != beam::HITS {
        beams.set(k + beam::HIT_OUT, beam::HIT_FOLLOWS | from);
        return;
    }
    let asked = ray::MISS_ARGS_ROOM;
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
    let face = beams.word(k + beam::HIT_FACE);
    let a = g.hit_ask_at(place);
    beams.set(a + beam::ASK_NODE, k + beam::HIT_NODE);
    beams.set(a + beam::ASK_FACE, face);
    beams.set(a + beam::ASK_LINE, beam::Grid::line(corner));
    beams.set(a + beam::ASK_TO, k + beam::HIT_OUT);
}

/// Each hit the walks again kept, a thread each as `hit_lead_cs`, once
/// the faces' pass asked what lies about each patch's face (raster.rs
/// `face_cs`): those faces put at its pixel as its last ray word
/// (`ray::FACES`), its own where it asked them, else those of the hit that
/// asked for it, reached through the neighbors each was told
/// (`beam::HIT_FOLLOWS`), a step left or up within the square each, so
/// two of its sides at most; no pixel is left waiting past the cast's
/// pass.
#[compute(threads(beam::HIT_LANES), entry_point_name = "hit_faces_cs")]
pub fn hit_faces_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(descriptor_set = 0, binding = 3)] rays: &Rays,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 9)] pyramid: &Pyramid,
) {
    if id.x >= hits(pyramid) {
        return;
    }
    let g = grid(eye);
    let k = g.hit_at(id.x);
    let px = beams.word(k + beam::HIT_PX);
    let mut faces = beams.word(k + beam::HIT_OUT);
    let mut steps = 0;
    while faces & beam::HIT_FOLLOWS != 0 && steps < 2 * beam::HIT_SQUARE {
        faces = beams.word(g.hit_at(faces & !beam::HIT_FOLLOWS) + beam::HIT_OUT);
        steps += 1;
    }
    // SAFETY: a pixel the walk again wrote, within the rays.
    unsafe { rays.write(UVec3::new(px & 0xffff, px >> 16, ray::FACES), faces) };
}
