// pxl-render/gpu/ao.rs

//! The screen-space occlusion: N8AO as the three.js build ran it until
//! 2026-09-14, brought over so it can be switched in beside the corner
//! occlusion a chunk shades itself with (chunk.wgsl's occlusion) and
//! compared. Five passes a frame while the setting is on: the world's depth
//! drawn once more from the eye, at one sample, into a depth the passes
//! can read whatever the frame's sample count; that depth to half
//! resolution with a normal (ao_downsample.wgsl); the occlusion at half
//! resolution (ao_sample.wgsl); the denoise over it, twice
//! (ao_blur.wgsl); and the occlusion back at the frame's resolution, one
//! byte a pixel (ao_upsample.wgsl), which every lit pass then reads at
//! group 0 binding 7 and applies before its grade (ao_map.wgsl), where the
//! post stack had it. The numbers are shader/ao_view.wgsl's.
//!
//! What it costs, at a frame of W x H pixels: the geometry once more with no
//! fragment stage; a full-resolution depth (4 bytes a pixel), the
//! half-resolution depth and normal (16 bytes a half-pixel, 4 a pixel), two
//! half-resolution occlusion images (4 bytes a half-pixel, 1 a pixel each),
//! and the full-resolution map (1 byte a pixel) — 11 bytes a pixel — and
//! three half-resolution and one full-resolution fragment pass. The blue
//! noise is 64 KB in the module.

use xpute_core::math::mat4::{mat4_invert, Mat4};

use crate::frame::shader::record::{PIPELINE_AO_BLUR, PIPELINE_AO_DOWNSAMPLE, PIPELINE_AO_SAMPLE, PIPELINE_AO_UPSAMPLE};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, pass_desc, release, texture_usage, word_bytes, GpuApi, NONE};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::pipeline::{Built, Pipelines};

/// Floats the Ao uniform (ao_view.wgsl's Ao) holds: proj, inv_proj, full,
/// half, fog_near, fog_far, padding.
const AO_FLOATS: usize = 40;
const FULL: usize = 32;
const HALF: usize = 34;
const FOG_NEAR: usize = 36;
const FOG_FAR: usize = 37;

/// N8AO's blue noise: 128 x 128, four channels of it, one a purpose — the
/// spin of the hemisphere, the push along its rays, and the two denoise
/// runs' spins.
const NOISE_SIDE: u32 = 128;
const NOISE: &[u8] = include_bytes!("../frame/shader/blue-noise.bin");

pub struct ScreenAo {
    /// A 1 x 1 map reading as unoccluded, bound where the frame's map is
    /// not yet made — a case's pass, and every lit pass before the first
    /// frame at a size.
    pub blank_view: u32,
    noise_view: u32,
    uniform: u32,
    uniform_words: [f32; AO_FLOATS],
    /// The denoise's two runs, at one stride each: a dynamic offset says
    /// which.
    run: u32,
    stride: u32,
    sampler: u32,
    downsample: Built,
    sample: Built,
    blur: Built,
    upsample: Built,
    width: u32,
    height: u32,
    /// The world's depth from the eye at one sample; its texture and view.
    depth: u32,
    pub depth_view: u32,
    /// Half resolution: the depth and normal, and the occlusion, twice.
    half: u32,
    half_view: u32,
    ping: [u32; 2],
    ping_view: [u32; 2],
    /// The occlusion at the frame's resolution, one byte a pixel.
    map: u32,
    map_view: u32,
    /// The groups: the downsample's, the sample's, the denoise's two
    /// (the first into ping[1] from ping[0], the second back), the
    /// upsample's.
    groups: [u32; 5],
}

impl ScreenAo {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines, align: u32) -> ScreenAo {
        let blank = object();
        (g.texture)(blank, 1, 1, 1, format::R8, 1, 1, texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST);
        (g.write_texture)(blank, 0, 1, 1, &[255]);
        let blank_view = object();
        (g.view)(blank_view, blank, dimension::D2, 0, 1, 0, 1);
        let noise = object();
        (g.texture)(noise, NOISE_SIDE, NOISE_SIDE, 1, format::RGBA8, 1, 1, texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST);
        (g.write_texture)(noise, 0, NOISE_SIDE, NOISE_SIDE, NOISE);
        let noise_view = object();
        (g.view)(noise_view, noise, dimension::D2, 0, 1, 0, 1);
        let uniform = object();
        (g.buffer)(uniform, word_bytes(AO_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let stride = 16u32.max(align);
        let run = object();
        (g.buffer)(run, stride * 2, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        (g.write_buffer)(run, 0, bytes(&[0.0f32, 0.0, 0.0, 0.0]));
        (g.write_buffer)(run, stride, bytes(&[1.0f32, 0.0, 0.0, 0.0]));
        let sampler = object();
        (g.sampler)(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::CLAMP, compare::NONE, 1);
        ScreenAo {
            blank_view,
            noise_view,
            uniform,
            uniform_words: [0.0; AO_FLOATS],
            run,
            stride,
            sampler,
            downsample: pipelines.build(g, PIPELINE_AO_DOWNSAMPLE, format::RGBA32F, 1),
            sample: pipelines.build(g, PIPELINE_AO_SAMPLE, format::CANVAS, 1),
            blur: pipelines.build(g, PIPELINE_AO_BLUR, format::CANVAS, 1),
            upsample: pipelines.build(g, PIPELINE_AO_UPSAMPLE, format::R8, 1),
            width: 0,
            height: 0,
            depth: 0,
            depth_view: 0,
            half: 0,
            half_view: 0,
            ping: [0; 2],
            ping_view: [0; 2],
            map: 0,
            map_view: 0,
            groups: [0; 5],
        }
    }

    /// The map as a lit pass's group entry at `binding`: the frame's where
    /// one is made, the blank before that.
    pub fn entry(&self, binding: u32) -> [u32; 5] {
        [binding, resource::VIEW, if self.map_view != 0 { self.map_view } else { self.blank_view }, 0, 0]
    }

    /// The blank map as a group entry at `binding`.
    pub fn blank_entry(&self, binding: u32) -> [u32; 5] {
        [binding, resource::VIEW, self.blank_view, 0, 0]
    }

    /// The camera as the passes read it — `proj` view to WebGPU clip — over
    /// a frame of `width` x `height` pixels with the fog band given.
    pub fn lay_out(&mut self, g: &GpuApi, proj: &Mat4, width: u32, height: u32, fog_near: f32, fog_far: f32) {
        let u = &mut self.uniform_words;
        let mut inv = [0.0; 16];
        mat4_invert(&mut inv, proj);
        for i in 0..16 {
            u[i] = proj[i] as f32;
            u[16 + i] = inv[i] as f32;
        }
        u[FULL] = width as f32;
        u[FULL + 1] = height as f32;
        u[HALF] = (width / 2) as f32;
        u[HALF + 1] = (height / 2) as f32;
        u[FOG_NEAR] = fog_near;
        u[FOG_FAR] = fog_far;
        (g.write_buffer)(self.uniform, 0, bytes(u));
    }

    /// The images at the frame's size, made where the size is new. Whether
    /// they were made, in which case the map every lit pass binds is a new
    /// object and their groups are to be made again.
    pub fn ensure(&mut self, g: &GpuApi, width: u32, height: u32) -> bool {
        if self.map != 0 && self.width == width && self.height == height {
            return false;
        }
        self.release_images();
        let (hw, hh) = ((width / 2).max(1), (height / 2).max(1));
        let attach = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        self.depth = object();
        (g.texture)(self.depth, width, height, 1, format::DEPTH32, 1, 1, attach);
        self.depth_view = object();
        (g.view)(self.depth_view, self.depth, dimension::D2, 0, 1, 0, 1);
        self.half = object();
        (g.texture)(self.half, hw, hh, 1, format::RGBA32F, 1, 1, attach);
        self.half_view = object();
        (g.view)(self.half_view, self.half, dimension::D2, 0, 1, 0, 1);
        for k in 0..2 {
            self.ping[k] = object();
            (g.texture)(self.ping[k], hw, hh, 1, format::CANVAS, 1, 1, attach);
            self.ping_view[k] = object();
            (g.view)(self.ping_view[k], self.ping[k], dimension::D2, 0, 1, 0, 1);
        }
        self.map = object();
        (g.texture)(self.map, width, height, 1, format::R8, 1, 1, attach);
        self.map_view = object();
        (g.view)(self.map_view, self.map, dimension::D2, 0, 1, 0, 1);
        self.width = width;
        self.height = height;

        let view = |binding: u32, id: u32| [binding, resource::VIEW, id, 0, 0];
        let sampler = [4, resource::SAMPLER, self.sampler, 0, 0];
        let uniform = buffer_entry(0, self.uniform);
        let run = [5, resource::BUFFER, self.run, 0, self.stride];
        let mut e = [0u32; 30];
        let g0 = object();
        e[..5].copy_from_slice(&uniform);
        e[5..10].copy_from_slice(&view(1, self.depth_view));
        (g.bind_group)(g0, self.downsample.layouts[0], &e[..10]);
        let g1 = object();
        e[5..10].copy_from_slice(&view(1, self.half_view));
        e[10..15].copy_from_slice(&view(2, self.noise_view));
        (g.bind_group)(g1, self.sample.layouts[0], &e[..15]);
        let mut blur = [0u32; 2];
        for (id, &src) in blur.iter_mut().zip(&self.ping_view) {
            *id = object();
            e[15..20].copy_from_slice(&view(3, src));
            e[20..25].copy_from_slice(&sampler);
            e[25..30].copy_from_slice(&run);
            (g.bind_group)(*id, self.blur.layouts[0], &e[..30]);
        }
        let g4 = object();
        e[5..10].copy_from_slice(&view(1, self.depth_view));
        e[10..15].copy_from_slice(&view(2, self.half_view));
        e[15..20].copy_from_slice(&view(3, self.ping_view[0]));
        e[20..25].copy_from_slice(&sampler);
        (g.bind_group)(g4, self.upsample.layouts[0], &e[..25]);
        self.groups = [g0, g1, blur[0], blur[1], g4];
        true
    }

    fn release_images(&mut self) {
        for id in [
            self.groups[0],
            self.groups[1],
            self.groups[2],
            self.groups[3],
            self.groups[4],
            self.map_view,
            self.map,
            self.ping_view[0],
            self.ping[0],
            self.ping_view[1],
            self.ping[1],
            self.half_view,
            self.half,
            self.depth_view,
            self.depth,
        ] {
            release(id);
        }
        self.groups = [0; 5];
        self.map = 0;
        self.map_view = 0;
    }

    /// Opens the depth pass from the eye; the caller draws the world into
    /// it with the prepass pipelines under the frame's group, and ends it.
    pub fn begin_prepass(&self, g: &GpuApi, encoder: u32) -> u32 {
        let pass = object();
        (g.pass)(pass, encoder, &pass_desc(0, 0, load::CLEAR, self.depth_view, load::CLEAR, 0.0, 0, NONE, NONE));
        pass
    }

    /// The four full-screen passes, from the depth the prepass drew to the
    /// map: the downsample into the half-resolution depth, the occlusion
    /// into ping[0], the denoise into ping[1] and back into ping[0], the
    /// upsample into the map.
    pub fn run(&self, g: &GpuApi, encoder: u32) {
        let steps: [(u32, u32, u32, u32); 5] = [
            (self.downsample.pipeline, self.groups[0], self.half_view, 0),
            (self.sample.pipeline, self.groups[1], self.ping_view[0], 0),
            (self.blur.pipeline, self.groups[2], self.ping_view[1], 0),
            (self.blur.pipeline, self.groups[3], self.ping_view[0], self.stride),
            (self.upsample.pipeline, self.groups[4], self.map_view, 0),
        ];
        for (k, &(pipeline, group, target, offset)) in steps.iter().enumerate() {
            let pass = object();
            (g.pass)(pass, encoder, &pass_desc(target, 0, load::CLEAR, 0, load::CLEAR, 0.0, 0, NONE, NONE));
            (g.set_pipeline)(pass, pipeline);
            if k == 2 || k == 3 {
                (g.set_bind_group)(pass, 0, group, &[offset]);
            } else {
                (g.set_bind_group)(pass, 0, group, &[]);
            }
            (g.draw)(pass, 3, 1, 0, 0);
            (g.end)(pass);
            release(pass);
        }
    }
}
