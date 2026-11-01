// pxl-shader/raster.rs

//! The samples' hits handed to the rasterizer (pxl-spec-core `raster`):
//! each stopped sample's box written for the raster's draw (`sample_box_cs`),
//! which pxl-render raster.wgsl draws and pxl-shader cast.rs `cast_cs`
//! reads each pixel's visible sample off; and what lies about each hit's
//! face asked once a hit (`face_cs`), a box's and a missed pixel's alike.

use pxl_spec_core::cast::End;
use pxl_spec_core::{beam, cast, raster, support, way};
use spirv_std::glam::{IVec4, UVec3};
use spirv_std::spirv;

use crate::beam::{node_at, Beamed, Beams, BeamAncestors, Words};
use crate::pass_observer::PassObserver;
use crate::eye::{eye_origin, eye_ray, grid, Eye};
use crate::light::Head;
use crate::observer::{self, Observer};
use crate::world::{face_of, unasked, NoPalette, World};
use crate::{add, opens};

/// Whether samples at words `a` and `b` stopped on the same box, alike in
/// every word a pixel reads of them: the node, the asset, whether it lacks
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
/// the raster's draw, the eye's and the light's alike: its box as the cast
/// stops on it (`cast::drawn`) from `origin`, appended to `boxes`, counted
/// in `args` (`raster::ARGS_INSTANCES`), its number and its axis,
/// `raster::EYED` where it lies about the eye at the screen's tangents
/// `tan` (`raster::is_eyed`), where `eyed` asks (the light's has none); none
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
    let (lo, hi) = cast::drawn(origin, &node, w.word(k + beam::SAMPLE_ASSET));
    let lacks = w.word(k + beam::SAMPLE_LACKS) != 0;
    let eyed = eyed && raster::is_eyed(lo, hi, tan);
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

/// The cell a ray from `origin` takes off the visible sample, the sample at
/// word `k` of `w`: where the ray enters it (`cast::entry`) across the axis
/// the sample entered by, the eye's pixels and the light's slots alike, and
/// whether it does.
pub(crate) fn taken<W: Words + ?Sized>(
    w: &W,
    k: u32,
    origin: [i32; 3],
    ray: &cast::Ray,
) -> (cast::Hit<u32>, bool) {
    let node = node_at(w, k + beam::SAMPLE_NODE);
    let asset = [
        w.word(k + beam::SAMPLE_ASSET),
        w.word(k + beam::SAMPLE_ASSET + 1),
    ];
    let (enter, axis, enters) = cast::entry(origin, ray, &node, asset[0]);
    let hit = cast::Hit {
        t: enter,
        axis,
        node,
        at: 0u32,
        asset,
    };
    (hit, enters && axis == w.word(k + beam::SAMPLE_AXIS))
}

/// Each sample of its observer's lattice, a thread each: written as a box
/// for the raster's draw (observer.rs `sample_box`), the screen's or the
/// shadow map's as its head names (pass_observer.rs `PassObserver`); and, the screen's,
/// what lies about its face asked once a box (`ask`), as its pixels draw
/// the face.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sample_box_cs"))]
pub fn sample_box_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let o = PassObserver::of(eye, head);
    let [across, down] = o.samples();
    if id.x >= across || id.y >= down {
        return;
    }
    let box_at = observer::sample_box(&o, &*beams, [id.x, id.y], boxes, args);
    if !o.light() && box_at != NONE {
        ask(eye, beams, [id.x, id.y], box_at);
        opens(args, raster::ARGS_FACES, box_at, raster::FACES_LANES);
    }
}

/// What lies about the face sample `s` entered, the screen's, whose box is
/// at `place` among the boxes: told at once where it needs no asking
/// (`world::unasked`, a lack none), left in its words (`beam::SAMPLE_FACES`)
/// where its pixels read it (cast.rs `cast_cs`); else its ask at `place`
/// (`beam::Grid::ask_at`), off the line its cell's rays resumed from, for
/// the faces' pass (`face_cs`), its faces to be left there.
fn ask(eye: &Eye, beams: &mut Beams, s: [u32; 2], place: u32) {
    let g = grid(eye);
    let k = g.sample_at(s);
    let a = g.ask_at(place);
    let (asks, faces) = if beams.word(k + beam::SAMPLE_LACKS) != 0 {
        (false, 0)
    } else {
        unasked(End::Hit, beams.word(k + beam::SAMPLE_ASSET))
    };
    if !asks {
        beams.set(k + beam::SAMPLE_FACES, faces);
        beams.set(a + beam::ASK_TO, beam::ASK_NONE);
        return;
    }
    let face = face_of(
        &eye_ray(eye, g.sample(s)),
        beams.word(k + beam::SAMPLE_AXIS),
    );
    let corner = beams.word(g.cell_at(g.sample_cell(s)) + beam::CELL_CORNER);
    beams.set(a + beam::ASK_NODE, k + beam::SAMPLE_NODE);
    beams.set(a + beam::ASK_FACE, face);
    beams.set(a + beam::ASK_LINE, beam::Grid::line(corner));
    beams.set(a + beam::ASK_TO, k + beam::SAMPLE_FACES);
}

/// Each hit asked its faces, a thread each, `raster::FACES_LANES` a
/// workgroup, dispatched off its args (`raster::ARGS_FACES`), each past
/// their count (`raster::ARGS_INSTANCES`) returning at once: what lies
/// about the face its ray entered (`World::faced`), asked of the world's
/// query once a hit rather than once a ray, off the line down to it from
/// the deepest node of its cell's beam ray's line over it, that line read
/// in place (`support::sub_line`, beam.rs `BeamAncestors`), and left in the word its ask
/// names (`beam::ASK_TO`). The boxes' asks (`beam::Grid::ask_at`), their
/// faces left in their samples' words, which the pixels read (cast.rs
/// `cast_cs`); and the missed pixels' hits' (`beam::Grid::hit_ask_at`;
/// cast.rs `hit_lead_cs`), their args laid alike (pxl-spec-core
/// `ray::MISS_ARGS_ROOM`), their faces left in the hits, which their pixels
/// take (cast.rs `hit_faces_cs`). One pass for both, over the args it is
/// bound to: where its asks lie (`raster::ARGS_ASKS`) and how many.
#[spirv(compute(threads(64, 1, 1), entry_point_name = "face_cs"))]
#[allow(clippy::too_many_arguments)]
pub fn face_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] clipmap: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 2)] ground: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 5)] shards: &[IVec4],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 6)] overwrites: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 7)] pool: &[u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &[u32],
) {
    if id.x >= args[raster::ARGS_INSTANCES as usize] {
        return;
    }
    let a = args[raster::ARGS_ASKS as usize] + id.x * beam::ASK_WORDS;
    let to = beams.word(a + beam::ASK_TO);
    if to == beam::ASK_NONE {
        return;
    }
    let faces = {
        let w: &Beams = beams;
        let (origin, _) = eye_origin(eye);
        let node = node_at(w, w.word(a + beam::ASK_NODE));
        let (zoom, c) = (node.zoom, node.idx(origin));
        let base = BeamAncestors::of(Beamed(w, w.word(a + beam::ASK_LINE)), origin);
        let mut world = World {
            clipmap,
            ground,
            palette_indices: NoPalette(0),
            shards,
            overwrites,
            eye,
            pool: way::tree::Read(pool),
        };
        let below = support::sub_line(&mut world, &base, zoom, c);
        world.faced(w.word(a + beam::ASK_FACE), zoom, c, &below.joined(&base))
    };
    beams.set(to, faces);
}
