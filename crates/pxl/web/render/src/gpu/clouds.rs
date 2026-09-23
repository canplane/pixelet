// pxl-render/gpu/clouds.rs

//! The clouds: a layer over the planet drawn after the opaque world
//! (frame/shader/clouds.wgsl), one full-screen pass reading the frame, the
//! air, and where the camera stands on the ground. What it costs is a
//! fragment a pixel where a ray meets the layer — eight octaves of value noise
//! and the air's lookups — and nothing where one does not.

use pxl_spec::space::pvf3::core::{IVEC_X_W, IVEC_Y_W};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::v3::V3;
use xpute_core::math::mat4::Mat4;

use crate::frame::shader::record::{PIPELINE_CLOUDS, PIPELINE_CLOUDS_ABOVE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, word_bytes, GpuApi};
use crate::gpu::api_spec::format;
use crate::gpu::atmosphere::AIR_WORDS;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::put_ray;
use crate::look::atmosphere::light_at;
use crate::look::environment::{ENV_AIR, ENV_MOON, ENV_SKY, ENV_SUN};

/// Floats the Clouds uniform (clouds.wgsl's Clouds) holds.
pub(crate) const CLOUDS_FLOATS: usize = 12 + 8 + 8;
pub(crate) const BASE: usize = 12;
pub(crate) const FRAC: usize = 14;
pub(crate) const TIME: usize = 16;
pub(crate) const COUNT: usize = 17;
pub(crate) const ALTITUDE: usize = 18;
pub(crate) const SUN_AT: usize = 20;
pub(crate) const MOON_AT: usize = 24;
/// The layer's height: a fair-weather cumulus base.
const ALTITUDE_KM: f64 = 1.5;
/// The cells a unit of the clouds' lattice spans, as a shift
/// (clouds.wgsl's CLOUD_LATTICE_CELLS).
pub(crate) const LATTICE_SHAMT: u32 = 12;
/// The lattice's count across the world, where it wraps: the world is the
/// X lane's width in cells (space/adapter/3857.rs).
const LATTICE_COUNT: u32 = 1 << (IVEC_X_W - LATTICE_SHAMT);
// Y's lane is as wide, so one count wraps both axes.
const _: () = assert!(IVEC_X_W == IVEC_Y_W, "one lattice count for both axes");

pub struct CloudPass {
    /// From under the layer, drawn only where the world left the far plane;
    /// from over it, over everything (frame/shader/clouds.wgsl).
    below: Built,
    above: Built,
    /// The camera over the layer this frame.
    over: bool,
    uniform: u32,
    group: u32,
    words: [f32; CLOUDS_FLOATS],
}

impl CloudPass {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, samples: u32, frame_buffer: u32, air: [u32; AIR_WORDS]) -> CloudPass {
        let below = pipelines.build(g, PIPELINE_CLOUDS, format::CANVAS, samples);
        let above = pipelines.build(g, PIPELINE_CLOUDS_ABOVE, format::CANVAS, samples);
        let uniform = object();
        (g.buffer)(uniform, word_bytes(CLOUDS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let group = object();
        let mut e = [0u32; 10 + AIR_WORDS];
        e[..5].copy_from_slice(&buffer_entry(0, frame_buffer));
        e[5..10].copy_from_slice(&buffer_entry(1, uniform));
        e[10..].copy_from_slice(&air);
        // One layout serves both pipelines: their groups are the same.
        (g.bind_group)(group, below.layouts[0], &e);
        CloudPass {
            below,
            above,
            over: false,
            uniform,
            group,
            words: [0.0; CLOUDS_FLOATS],
        }
    }

    pub fn set_samples(&mut self, g: &GpuApi, pipelines: &mut Pipelines, samples: u32) {
        self.below = pipelines.build(g, PIPELINE_CLOUDS, format::CANVAS, samples);
        self.above = pipelines.build(g, PIPELINE_CLOUDS_ABOVE, format::CANVAS, samples);
    }

    /// The frame's clouds: `view_proj`, the root frame to WebGPU clip, and
    /// the eye in it; the cell the root frame's origin is on the map, as the
    /// integer lanes of its PVF3; and the environment's lane `env`, for the
    /// sky's clock and the sun's and the moon's directions, whose light
    /// reaches the layer through the air.
    ///
    /// The origin is split on the lattice here, in its own lanes, so the
    /// shader never holds a coordinate the size of the world: it gets the
    /// lattice index, the lane shifted down, and the fraction of a unit past
    /// it, the lane's low bits, and adds only the few thousand cells between
    /// the origin and the layer. A shift and a mask on two's complement lanes
    /// are a floor and a remainder, so a cell west of the origin's zero lands
    /// in the lattice unit it is in, and the index masked to the count is the
    /// same unit wherever the lane wrapped.
    pub fn lay_out(&mut self, g: &GpuApi, view_proj: &Mat4, eye: &V3, origin: IVec, env: &[f32]) {
        put_ray(&mut self.words, 0, view_proj, eye);
        let unit = ivec::sh(origin, -(LATTICE_SHAMT as i32));
        let within = ivec::and(origin, ivec::fill((1 << LATTICE_SHAMT) - 1));
        for (k, lane) in [ivec::X, ivec::Y].into_iter().enumerate() {
            self.words[BASE + k] = f32::from_bits(lane(unit) as u32 & (LATTICE_COUNT - 1));
            self.words[FRAC + k] = lane(within) as f32 / (1u32 << LATTICE_SHAMT) as f32;
        }
        self.words[TIME] = env[ENV_SKY + 1];
        self.words[COUNT] = f32::from_bits(LATTICE_COUNT);
        self.words[ALTITUDE] = ALTITUDE_KM as f32;
        self.over = env[ENV_AIR] as f64 > ALTITUDE_KM;
        for (at, toward) in [(SUN_AT, ENV_SUN), (MOON_AT, ENV_MOON)] {
            for (w, v) in self.words[at..at + 3].iter_mut().zip(light_at(ALTITUDE_KM, env[toward + 2] as f64)) {
                *w = v as f32;
            }
        }
        (g.write_buffer)(self.uniform, 0, bytes(&self.words));
    }

    /// The layer into the open pass, over what is drawn opaque.
    pub fn draw(&self, g: &GpuApi, pass: u32) {
        (g.set_pipeline)(pass, if self.over { self.above.pipeline } else { self.below.pipeline });
        (g.set_bind_group)(pass, 0, self.group, &[]);
        (g.draw)(pass, 3, 1, 0, 0);
    }
}
