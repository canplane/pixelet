// pxl-render/pass/ao.rs

//! Screen-space ambient occlusion, after N8AO. The occlusion is a map over the screen, as the
//! light's shadow map is over the world. `run` finds it at half resolution from the depth the
//! static opaque draws laid: it downsamples the depth, samples the occlusion and denoises it
//! twice. The renderer runs it only in a frame where what it reads changed, and the map is held
//! across frames. Every drawn frame, `apply` multiplies the map into the world's color in the
//! damaged tiles, as a draw of the world's pass. The world's pass is ended and begun again
//! around `run` only in a frame that finds the map again. The images take about 3 bytes a
//! pixel.

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
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::put_mat;
use crate::gpu::span_spec::GpuSpanName;

/// N8AO's blue noise, `AO_NOISE_SIDE` texels a side, which a pixel reads tiled. Its four
/// channels hold the spin of the hemisphere's samples, the offset of their distances along
/// their directions, and the spins of the two denoise passes.
const NOISE: &[u8] = include_bytes!("blue_noise.bin");

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
    half: u32,
    half_view: u32,
    /// The targets of the two denoise passes, which write into each other. `ping[0]` holds
    /// the map, which is kept across frames.
    ping: [u32; 2],
    ping_view: [u32; 2],
    /// The groups of the four passes in their order: downsample, sample, denoise from
    /// `ping[0]` into `ping[1]`, and denoise back. The downsample and the sample read the
    /// world's depth.
    groups: [Group; 4],
    /// The group `apply` draws by, over the map and the screen's tiles (`set_apply_group`).
    apply_group: Group,
}

impl ScreenAo {
    pub fn new(g: &Gpu, pipelines: &mut PipelineCache, align: u32, frame_buf: u32) -> ScreenAo {
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
            half: 0,
            half_view: 0,
            ping: [0; 2],
            ping_view: [0; 2],
            groups: Default::default(),
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

    /// The words the passes read, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.uniform_words
    }

    /// Writes `words` to the device. The frame calls it only when its read found them
    /// changed.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.uniform_words));
    }

    /// Makes the half-resolution images again when the size is another, and sets the passes'
    /// groups over them and the world's depth view `depth_view`. Returns true when a group was
    /// made anew: the map is then to be found again.
    pub fn ensure(&mut self, g: &Gpu, width: u32, height: u32, depth_view: u32) -> bool {
        if self.half == 0 || self.width != width || self.height != height {
            self.release_images();
            let (hw, hh) = (
                (width / AO_DOWNSCALE).max(1),
                (height / AO_DOWNSCALE).max(1),
            );
            let attach = texture_usage::RENDER_ATTACHMENT | texture_usage::TEXTURE_BINDING;
            self.half = g.texture(hw, hh, 1, Format::r32float, 1, attach);
            self.half_view = g.view(self.half, false, 0, 1, 0, 1);
            for k in 0..2 {
                self.ping[k] = g.texture(hw, hh, 1, g.canvas(), 1, attach);
                self.ping_view[k] = g.view(self.ping[k], false, 0, 1, 0, 1);
            }
            self.width = width;
            self.height = height;
        }

        let view = view_entry;
        let sampler = sampler_entry(4, self.sampler);
        let run = buf_range_entry(5, self.run, 0, self.stride);
        // The frame's words go at binding 0 in the passes that read the screen's size, and the
        // occlusion's own words at binding 6.
        let uniform = buf_entry(6, self.uniform);
        let group = |layout: Layout, frame: bool, between: &[Entry]| {
            let frame = frame.then(|| buf_entry(0, self.frame_buf));
            let e: Vec<Entry> = frame.iter().chain(between).copied().collect();
            (layout, e)
        };
        let g0 = group(self.downsample.layouts[0], true, &[view(1, depth_view)]);
        let g1 = group(
            self.sample.layouts[0],
            true,
            &[
                view(1, self.half_view),
                view(2, self.noise_view),
                view(3, depth_view),
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
        let [b0, b1] = blur;
        let mut made = false;
        for (group, (layout, e)) in self.groups.iter_mut().zip([g0, g1, b0, b1]) {
            made |= group.set(g, layout, &e);
        }
        made
    }

    fn release_images(&mut self) {
        for id in [
            self.ping_view[0],
            self.ping[0],
            self.ping_view[1],
            self.ping[1],
            self.half_view,
            self.half,
        ] {
            release(id);
        }
        self.half = 0;
        self.half_view = 0;
    }

    pub fn run(&self, ctx: &mut RenderContext) {
        let (g, encoder) = (ctx.g, ctx.encoder);
        let stamp = ctx.stamp(GpuSpanName::ao);
        let steps: [(u32, u32, u32, u32); 4] = [
            (self.downsample.id, self.groups[0].id(), self.half_view, 0),
            (self.sample.id, self.groups[1].id(), self.ping_view[0], 0),
            (self.blur.id, self.groups[2].id(), self.ping_view[1], 0),
            (
                self.blur.id,
                self.groups[3].id(),
                self.ping_view[0],
                self.stride,
            ),
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

    /// Makes the group `apply` draws by, over the half-resolution depth, the map and the
    /// screen's tiles, of which `tiles` holds the words and the uniform. It is called before
    /// the frame's commands, once `ensure` has made the images.
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

    /// Multiplies the map into the world's color at each pixel, as a draw in the world's pass
    /// `pass` after the static opaque draws (ao_apply.wgsl). It draws a quad for each of the
    /// `cnt` tiles, and the quad of a tile the group's tiles leave undamaged collapses.
    /// Every drawn frame calls it, whether or not the map was found again, as the world's
    /// color is drawn again in the damaged tiles each frame.
    pub fn apply(&self, pass: &mut TrackedRenderPass, cnt: u32) {
        pass.set_pipeline(self.apply.id);
        pass.set_bind_group(0, self.apply_group.id(), &[]);
        pass.draw(6, cnt, 0, 0);
    }
}
