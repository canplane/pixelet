// pxl-shader/raster.rs

//! The samples' hits handed to the rasterizer (pxl-engine `raster`):
//! each stopped sample's box written for the raster's draw (`sample_box_cs`),
//! which pxl-render raster.wgsl draws and pxl-render cast.wgsl `cast_cs`
//! reads each pixel's visible sample off; and the ask of what lies about each
//! box's face, which pxl-render face.wgsl `face_cs` answers once a box.

use pxl_engine::cast::End;
use pxl_engine::{beam, cast, raster};
use spirv_std::glam::UVec3;
use spirv_std::spirv;

use crate::beam::{node_at, Beams, Words};
use crate::eye::{eye_ray, grid, Eye};
use crate::light::{Head, Map};
use crate::observer::{self, Level, Observer};
use crate::world::{face_of, unasked_face};
use crate::{add, opens};

/// Whether samples at words `a` and `b` stopped on the same box, alike in
/// every word a pixel reads of them: the node, the asset, whether it lacks
/// and the axis entered across, so what lies about its face is alike too
/// (pxl-render face.wgsl `face_word`, its face the axis's turned to the eye).
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

/// No sample before it (`write_box`).
pub(crate) const NONE: u32 = u32::MAX;

/// The sample at word `k` of `w`, numbered `sample`, written as a box for
/// the raster's draw, the eye's and the light's alike: its box as the cast
/// stops on it (`cast::asset_box`) from `origin`, appended to `boxes`, counted
/// in `args` (`raster::ARGS_INSTANCES`), its number and its axis,
/// `raster::EYED` where it lies about the eye at the screen's tangents
/// `tan` (`raster::is_eyed`), where `eyed` asks (the light's has none); none
/// where it went on to its reach, or where a sample at `before`, the one
/// across or up, `NONE` where there is none, stopped alike (`alike`), which
/// some sample before it in turn draws. Its place among the boxes, `NONE`
/// where it wrote none.
pub(crate) fn write_box<W: Words + ?Sized>(
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
    let (lo, hi) = cast::asset_box(origin, &node, w.word(k + beam::SAMPLE_ASSET));
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

/// Each sample of the camera grid, a thread each: written as a box for the
/// raster's draw (observer.rs `sample_box`); and what lies about its face
/// asked once a box (`ask`), as its pixels draw the face. The shadow map's
/// is `light_sample_box_cs`, which asks none.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "sample_box_cs"))]
pub fn sample_box_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    let (s, box_at) = box_by(eye, id, beams, boxes, args);
    if box_at != NONE {
        ask(eye, beams, s, box_at);
        opens(args, raster::ARGS_FACES, box_at, raster::FACES_LANES);
    }
}

/// `sample_box_cs` for the shadow map's samples.
#[spirv(compute(threads(8, 8, 1), entry_point_name = "light_sample_box_cs"))]
pub fn light_sample_box_cs(
    #[spirv(global_invocation_id)] id: UVec3,
    #[spirv(uniform, descriptor_set = 0, binding = 0)] eye: &Eye,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 8)] beams: &mut Beams,
    #[spirv(uniform, descriptor_set = 0, binding = 11)] head: &Head,
    #[spirv(storage_buffer, descriptor_set = 0, binding = 14)] boxes: &mut [u32],
    #[spirv(storage_buffer, descriptor_set = 0, binding = 16)] args: &mut [u32],
) {
    box_by(&Map::of(eye, head), id, beams, boxes, args);
}

/// Sample `id` of `o`'s lattice written as a box, where it has one: the
/// sample, and where its box lies among the boxes, `NONE` where none.
fn box_by<O: Observer>(
    o: &O,
    id: UVec3,
    beams: &mut Beams,
    boxes: &mut [u32],
    args: &mut [u32],
) -> ([u32; 2], u32) {
    let [across, down] = o.samples();
    let s = o.take(&*beams, Level::Sample, [id.x, id.y]);
    if s[0] >= across || s[1] >= down {
        return (s, NONE);
    }
    (s, observer::sample_box(o, &*beams, s, boxes, args))
}

/// What lies about the face sample `s` entered, the screen's, whose box is
/// at `place` among the boxes: told at once where it needs no asking
/// (`world::unasked_face`, a lack none), left in its words (`beam::SAMPLE_FACES`)
/// where its pixels read it (pxl-render cast.wgsl `cast_cs`); else its ask at `place`
/// (`beam::Grid::ask_at`), off the line its cell's rays resumed from, for
/// the faces' pass (pxl-render face.wgsl `face_cs`), its faces to be left there.
fn ask(eye: &Eye, beams: &mut Beams, s: [u32; 2], place: u32) {
    let g = grid(eye);
    let k = g.sample_at(s);
    let a = g.ask_at(place);
    let (asks, faces) = if beams.word(k + beam::SAMPLE_LACKS) != 0 {
        (false, 0)
    } else {
        unasked_face(End::Hit, beams.word(k + beam::SAMPLE_ASSET))
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
