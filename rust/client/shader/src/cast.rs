// pxl-shader/cast.rs

//! What lies about the faces the pixels pxl-render cast.wgsl `cast_cs` marked
//! stopped on once walked again (pxl-render miss_hit.wgsl, miss.wgsl), asked
//! once a hit, as a box's is (`hit_lead_cs`, pxl-render face.wgsl `face_cs`),
//! and put at their pixels (`hit_faces_cs`); and the tally and the list the
//! light's slots are counted and listed by as the eye's pixels are.

use pxl_engine::{beam, pyramid, raster, ray};
use pxl_shader_macro::compute;
use spirv_std::glam::UVec3;

use super::Rays;
use crate::beam::{Beams, Words};
use crate::eye::{grid, Eye};
use crate::pyramid::{pyramid_at, Pyramid};
use crate::{add, opens};

/// The tangents across and up of where drawn pixel `p`'s ray goes through it,
/// its middle off by the eye's jitter (pxl-engine `eye::JITTER`), of the
/// `size` the rays are, row 0 at the top, as the screen pass forms its own
/// pixel's: the screen's half-angles' scaled by where it lies.
pub(crate) fn px_tan(eye: &Eye, p: UVec3, size: UVec3) -> [f32; 2] {
    let ndc_x = (p.x as f32 + 0.5 + eye.jitter.x) / size.x as f32 * 2.0 - 1.0;
    let ndc_y = 1.0 - (p.y as f32 + 0.5 + eye.jitter.y) / size.y as f32 * 2.0;
    [ndc_x * eye.tan.x, ndc_y * eye.tan.y]
}

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

/// The missed pixels' hits the walks again kept (pxl-render miss.wgsl `keep_hit`), as many as they
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

/// Each hit the walks again kept (pxl-render miss.wgsl `keep_hit`), a thread each,
/// `beam::HIT_LANES` a workgroup, dispatched off their count
/// (`pyramid::HITS_ARGS`): the one that asks what lies about its face is
/// one whose pixel's neighbors to the left and above wait for no hit alike
/// (`alike`) within its square of `beam::HIT_SQUARE` a side, so a patch of
/// hits alike is asked once a square, and which of them asks is a function
/// of the pixels alone. It is listed with
/// its ask (`beam::Grid::hit_ask_at`) for the faces' pass as a box is
/// (pxl-render face.wgsl `face_cs`; `ray::MISS_ARGS_ROOM` of the walk again's words,
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
/// the faces' pass asked what lies about each patch's face (pxl-render
/// face.wgsl `face_cs`): those faces put at its pixel as its last ray word
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
