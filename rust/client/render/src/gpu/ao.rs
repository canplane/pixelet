// pxl-render/gpu/ao.rs

//! Screen-space occlusion (N8AO), a field over the screen as the light's map is over the
//! world: found from the world's own depth once the static opaque draws laid it (downsample,
//! sample, denoise twice, at half resolution), only where what it reads moved, and held;
//! brought to the pixel and laid over the world's color in the claimed tiles each drawn frame
//! (`apply`), as the tiles' quads, a draw of the world's pass, which ends about it only where
//! the map is found again. About 3 bytes a pixel.

use pxl_spec::space::coord::mat4::{self, Mat4};

use crate::frame::shader::record::{
    PIPELINE_AO_APPLY, PIPELINE_AO_BLUR, PIPELINE_AO_DOWNSAMPLE, PIPELINE_AO_SAMPLE,
};
use crate::frame::shader::word::{ao, AO_DOWNSCALE, AO_FLOATS, AO_NOISE_SIDE};
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{
    buf_entry, buf_range_entry, buffer_usage, bytes, pass_desc, release, sampler_entry,
    texture_usage, view_entry, word_bytes, Entry, Format, Gpu, Group, Layout, Load, RenderContext,
    TrackedRenderPass,
};
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::span_spec::GpuSpanName;

/// N8AO's blue noise, `AO_NOISE_SIDE` a side; its four channels are the
/// hemisphere spin, the ray push and the two denoise spins.
const NOISE: &[u8] = include_bytes!("../frame/shader/blue-noise.bin");

pub struct ScreenAo {
    noise_view: u32,
    uniform: u32,
    uniform_words: [f32; AO_FLOATS],
    frame_buf: u32,
    run: u32,
    stride: u32,
    sampler: u32,
    downsample: Pipeline,
    sample: Pipeline,
    blur: Pipeline,
    apply: Pipeline,
    width: u32,
    height: u32,
    /// The world's depth, read only, its view as last bound.
    depth_view: u32,
    half: u32,
    half_view: u32,
    /// The denoise's two targets; the map, held across frames, is ping[0].
    ping: [u32; 2],
    ping_view: [u32; 2],
    /// Downsample, sample, denoise ping[0]->ping[1], denoise back.
    groups: [u32; 4],
    /// The apply's, over the map and the screen's tiles (`set_apply_group`).
    apply_group: Group,
}

impl ScreenAo {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, align: u32, frame_buf: u32) -> ScreenAo {
        let noise = g.texture(
            AO_NOISE_SIDE,
            AO_NOISE_SIDE,
            1,
            Format::rgba8unorm,
            1,
            texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST,
        );
        g.write_texture(noise, 0, AO_NOISE_SIDE, AO_NOISE_SIDE, NOISE);
        let noise_view = g.view(noise, false, 0, 1, 0, 1);
        let uniform = g.buf(
            word_bytes(AO_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let stride = 16u32.max(align);
        let run = g.buf(stride * 2, buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        g.write_buf(run, 0, bytes(&[0.0f32, 0.0, 0.0, 0.0]));
        g.write_buf(run, stride, bytes(&[1.0f32, 0.0, 0.0, 0.0]));
        let sampler = g.sampler(
            FilterMode::linear,
            FilterMode::linear,
            MipmapFilterMode::nearest,
            AddressMode::clamp_to_edge,
            None,
            1,
        );
        ScreenAo {
            noise_view,
            uniform,
            uniform_words: [0.0; AO_FLOATS],
            frame_buf,
            run,
            stride,
            sampler,
            downsample: pipelines.build(g, PIPELINE_AO_DOWNSAMPLE, Format::r32float),
            sample: pipelines.build(g, PIPELINE_AO_SAMPLE, g.canvas()),
            blur: pipelines.build(g, PIPELINE_AO_BLUR, g.canvas()),
            apply: pipelines.build_world(g, PIPELINE_AO_APPLY),
            width: 0,
            height: 0,
            depth_view: 0,
            half: 0,
            half_view: 0,
            ping: [0; 2],
            ping_view: [0; 2],
            groups: [0; 4],
            apply_group: Group::default(),
        }
    }

    pub fn prepare(&mut self, proj: &Mat4, width: u32, height: u32) {
        let u = &mut self.uniform_words;
        let mut inv = [0.0; 16];
        mat4::invert(&mut inv, proj);
        put_mat(u, ao::PROJ, proj);
        put_mat(u, ao::INV_PROJ, &inv);
        u[ao::HALF] = (width / AO_DOWNSCALE) as f32;
        u[ao::HALF + 1] = (height / AO_DOWNSCALE) as f32;
    }

    /// The words its passes read, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.uniform_words
    }

    /// `words` written, where the frame's read found them moved.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.uniform_words));
    }

    /// Its images at half the world's size, its groups over the world's
    /// depth `depth_view`, read only: true when they were made again, so the
    /// map holds nothing yet.
    pub fn ensure(&mut self, g: &Gpu, width: u32, height: u32, depth_view: u32) -> bool {
        if self.half != 0
            && self.width == width
            && self.height == height
            && self.depth_view == depth_view
        {
            return false;
        }
        self.release_images();
        let (hw, hh) = (
            (width / AO_DOWNSCALE).max(1),
            (height / AO_DOWNSCALE).max(1),
        );
        let attach = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
        self.depth_view = depth_view;
        self.half = g.texture(hw, hh, 1, Format::r32float, 1, attach);
        self.half_view = g.view(self.half, false, 0, 1, 0, 1);
        for k in 0..2 {
            self.ping[k] = g.texture(hw, hh, 1, g.canvas(), 1, attach);
            self.ping_view[k] = g.view(self.ping[k], false, 0, 1, 0, 1);
        }
        self.width = width;
        self.height = height;

        let view = view_entry;
        let sampler = sampler_entry(4, self.sampler);
        let run = buf_range_entry(5, self.run, 0, self.stride);
        // The frame's at 0 where a pass reads the screen's size; the AO's uniform at 6 where it
        // reads its words.
        let uniform = buf_entry(6, self.uniform);
        let group = |layout: Layout, frame: bool, between: &[Entry]| {
            let frame = frame.then(|| buf_entry(0, self.frame_buf));
            let e: Vec<Entry> = frame.iter().chain(between).copied().collect();

            g.bind_group(layout, &e)
        };
        let g0 = group(
            self.downsample.layouts[0],
            true,
            &[view(1, self.depth_view)],
        );
        let g1 = group(
            self.sample.layouts[0],
            true,
            &[
                view(1, self.half_view),
                view(2, self.noise_view),
                view(3, self.depth_view),
                uniform,
            ],
        );
        let blur = self.ping_view.map(|src| {
            group(
                self.blur.layouts[0],
                false,
                &[
                    view(1, self.half_view),
                    view(2, self.noise_view),
                    view(3, src),
                    sampler,
                    run,
                    uniform,
                ],
            )
        });
        self.groups = [g0, g1, blur[0], blur[1]];
        true
    }

    fn release_images(&mut self) {
        for id in [
            self.groups[0],
            self.groups[1],
            self.groups[2],
            self.groups[3],
            self.ping_view[0],
            self.ping[0],
            self.ping_view[1],
            self.ping[1],
            self.half_view,
            self.half,
        ] {
            release(id);
        }
        self.groups = [0; 4];
        self.half = 0;
        self.half_view = 0;
    }

    pub fn run(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::ao);
        let steps: [(u32, u32, u32, u32); 4] = [
            (self.downsample.id, self.groups[0], self.half_view, 0),
            (self.sample.id, self.groups[1], self.ping_view[0], 0),
            (self.blur.id, self.groups[2], self.ping_view[1], 0),
            (self.blur.id, self.groups[3], self.ping_view[0], self.stride),
        ];
        for (k, &(pipeline, group, target, off)) in steps.iter().enumerate() {
            let pass = g.pass(
                encoder,
                &pass_desc(target, Load::Clear, 0, Load::Clear, 0.0, stamp.pass()),
            );
            g.set_pipeline(pass, pipeline);
            if k == 2 || k == 3 {
                g.set_bind_group(pass, 0, group, &[off]);
            } else {
                g.set_bind_group(pass, 0, group, &[]);
            }
            g.draw(pass, 3, 1, 0, 0);
            g.end(pass);
        }
    }

    /// The apply's group over its images and the tiles `tiles` (their words
    /// and uniform), before the frame's commands, its images made
    /// (`ensure`).
    pub fn set_apply_group(&mut self, g: &Gpu, tiles: (u32, u32)) {
        self.apply_group.set(
            g,
            self.apply.layouts[0],
            &[
                view_entry(1, self.half_view),
                view_entry(2, self.ping_view[0]),
                buf_entry(6, self.uniform),
                buf_entry(30, tiles.0),
                buf_entry(31, tiles.1),
            ],
        );
    }

    /// The map brought to each pixel and multiplied into the world's color,
    /// a draw of the world's pass `pass` once the static opaque draws laid
    /// it, in the tiles its group binds (`set_apply_group`) claim of
    /// `count`, a quad each (ao_apply.wgsl): every drawn frame, as the
    /// world's color is drawn again there, whether or not the map was found
    /// again.
    pub fn apply(&self, pass: &mut TrackedRenderPass, count: u32) {
        pass.set_pipeline(self.apply.id);
        pass.set_bind_group(0, self.apply_group.id(), &[]);
        pass.draw(6, count, 0, 0);
    }
}
