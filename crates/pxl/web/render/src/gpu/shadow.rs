// pxl-render/gpu/shadow.rs

//! The sun's shadow map: the world drawn once more each frame from the sun,
//! depth only, into a square map over the map center, which every lit pass
//! then compares its fragments against (shader/shadow.wgsl). The light's frame
//! is a second frame uniform whose view_proj is the light's, so each pass
//! draws its depth with the vertex shader it already has.
//!
//! Orthographic, 400 units across at the sun's angle, near enough for a texel
//! to be a fifth of a unit on a 2048 map: further than that the map's texels
//! grow past a cell and the shadow reads as blocks. What the map costs is its
//! texels times four bytes, once: 16 MB at 2048, 4 MB at 1024, the phone's.
//!
//! The map and its sampler sit at group 0 bindings 1 and 2 in every pass that
//! reads them. A pass with no shadow — a case, or the depth pass itself, which
//! cannot read the map it writes — binds a blank map, where every fragment
//! reads as lit.

use pxl_spec::space::mat4::{mat4, mat4_look_at, mat4_mul, Mat4};
use pxl_spec::space::v3::V3;

use crate::frame::shader::record::FRAME_FLOATS;
use crate::gpu::api::{buffer_usage, bytes, object, pass_desc, texture_usage, word_bytes, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::renderer::{AIR_AT_VERTICES, CAMERA_POS, GLOBE, PLANET, VIEW_PROJ};

const HALF_EXTENT: f64 = 200.0;
const LIGHT_DISTANCE: f64 = 1200.0;
const LIGHT_FAR: f64 = 2400.0;

fn map_size(handheld: bool) -> u32 {
    if handheld {
        1024
    } else {
        2048
    }
}

/// An orthographic projection into WebGPU clip space, z in [0, 1] from near
/// to far, column-major.
fn ortho(out: &mut Mat4, half: f64, near: f64, far: f64) {
    *out = [0.0; 16];
    out[0] = 1.0 / half;
    out[5] = 1.0 / half;
    out[10] = -1.0 / (far - near);
    out[14] = -near / (far - near);
    out[15] = 1.0;
}

pub struct Shadow {
    map_view: u32,
    sampler: u32,
    blank_view: u32,
    blank_sampler: u32,
    /// The light's frame: the frame uniform with the light's view_proj.
    pub light_frame: u32,
    light: [f32; FRAME_FLOATS],
    pub light_view_proj: Mat4,
    /// One texel of the map, in units: the normal offset a fragment takes.
    pub texel: f64,
    pub bytes: u32,
}

impl Shadow {
    pub fn new(g: &Gpu, handheld: bool) -> Shadow {
        let size = map_size(handheld);
        let map = object();
        g.texture(map, size, size, 1, format::DEPTH32, 1, texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING);
        let map_view = object();
        g.view(map_view, map, dimension::D2, 0, 1, 0, 1);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::CLAMP, compare::LESS_EQUAL, 1);
        let blank = object();
        g.texture(blank, 1, 1, 1, format::DEPTH32, 1, texture_usage::TEXTURE_BINDING);
        let blank_view = object();
        g.view(blank_view, blank, dimension::D2, 0, 1, 0, 1);
        let blank_sampler = object();
        g.sampler(blank_sampler, filter::NEAREST, filter::NEAREST, filter::NEAREST, address::CLAMP, compare::LESS_EQUAL, 1);
        let light_frame = object();
        g.buffer(light_frame, word_bytes(FRAME_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        Shadow {
            map_view,
            sampler,
            blank_view,
            blank_sampler,
            light_frame,
            light: [0.0; FRAME_FLOATS],
            light_view_proj: mat4(),
            texel: HALF_EXTENT * 2.0 / size as f64,
            bytes: size * size * 4,
        }
    }

    /// The map and its sampler as a group's entries, at `binding` and the next.
    pub fn entries(&self, binding: u32) -> [u32; 10] {
        [binding, resource::VIEW, self.map_view, 0, 0, binding + 1, resource::SAMPLER, self.sampler, 0, 0]
    }

    /// A map nothing is drawn into, and a sampler, in the same place.
    pub fn blank_entries(&self, binding: u32) -> [u32; 10] {
        [binding, resource::VIEW, self.blank_view, 0, 0, binding + 1, resource::SAMPLER, self.blank_sampler, 0, 0]
    }

    /// Aims the light from `sun_dir` at `target`, both in the root frame, and
    /// writes the light's frame, with the frame's curve and globe: the map
    /// drawn into the shadow is the one the eye sees laid on the planet, or a
    /// far caster would shade the ground it no longer stands over.
    pub fn aim(&mut self, g: &Gpu, sun_dir: &[f32], target: V3, globe: [f32; 8]) {
        let eye = [
            target[0] + sun_dir[0] as f64 * LIGHT_DISTANCE,
            target[1] + sun_dir[1] as f64 * LIGHT_DISTANCE,
            target[2] + sun_dir[2] as f64 * LIGHT_DISTANCE,
        ];
        let (mut view, mut proj) = (mat4(), mat4());
        mat4_look_at(&mut view, eye, target, [0.0, 0.0, 1.0]);
        ortho(&mut proj, HALF_EXTENT, 0.1, LIGHT_FAR);
        mat4_mul(&mut self.light_view_proj, &proj, &view);
        for (i, v) in self.light_view_proj.iter().enumerate() {
            self.light[VIEW_PROJ + i] = *v as f32;
        }
        self.light[CAMERA_POS..CAMERA_POS + 3].copy_from_slice(&[eye[0] as f32, eye[1] as f32, eye[2] as f32]);
        self.light[PLANET..PLANET + 4].copy_from_slice(&globe[..4]);
        self.light[GLOBE..GLOBE + 4].copy_from_slice(&globe[4..]);
        // Depth and nothing else: no vertex takes the air (frame.wgsl's
        // air_at_vertices).
        self.light[AIR_AT_VERTICES] = 0.0;
        g.write_buffer(self.light_frame, 0, bytes(&self.light));
    }

    /// Opens the depth pass over the map, timed under `stamp`; the caller
    /// draws into it and ends it.
    pub fn begin(&self, g: &Gpu, encoder: u32, stamp: &mut Stamp) -> u32 {
        let pass = object();
        g.pass(pass, encoder, &pass_desc(0, load::CLEAR, self.map_view, load::CLEAR, 1.0, stamp.pass()));
        pass
    }
}
