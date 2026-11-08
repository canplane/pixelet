// pxl-render/gpu/sky.rs

//! Draws the sky at the far plane after the opaque world, so only the pixels the world left
//! empty run the sky's shader: the air toward the eye, the sun's and the moon's discs and the
//! moon's map.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::mat4::{self, Mat4};

use crate::frame::shader::record::PIPELINE_SKY;
use crate::frame::shader::word::{sky, SKY_FLOATS};
use crate::gpu::api::{
    buf_entry, buffer_usage, bytes, sampler_entry, view_entry, word_bytes, Gpu, TrackedRenderPass,
};
use crate::gpu::atmosphere::Atmosphere;
use crate::gpu::image_map::ImageMap;
use crate::gpu::pipeline::{Pipeline, Pipelines};
use crate::look::environment::ENV_POLE;

pub struct SkyPass {
    pipeline: Pipeline,
    uniform: u32,
    group: u32,
    words: [f32; SKY_FLOATS],
}

impl SkyPass {
    /// `moon` is the moon's map, and `atmosphere` the air the sky is lit through.
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        frame_buf: u32,
        moon: &ImageMap,
        atmosphere: &Atmosphere,
    ) -> SkyPass {
        let pipeline = pipelines.build_world(g, PIPELINE_SKY);
        let uniform = g.buf(
            word_bytes(SKY_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let mut entries = vec![
            buf_entry(0, frame_buf),
            buf_entry(19, uniform),
            view_entry(2, moon.view),
            sampler_entry(3, moon.sampler),
        ];
        entries.extend(atmosphere.sky_entries());
        entries.extend(atmosphere.entries());
        let group = g.bind_group(pipeline.layouts[0], &entries);
        SkyPass {
            pipeline,
            uniform,
            group,
            words: [0.0; SKY_FLOATS],
        }
    }

    /// The words the sky is drawn by, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// Lays out the sky's words: the rays under the screen by `view_proj`, the pole the sky
    /// turns about from `env`, and whether the moon's map has landed.
    pub fn prepare(&mut self, view_proj: &Mat4, env: &[f32], moon_mapped: bool) {
        put_ray(&mut self.words, sky::RAY, view_proj);
        self.words[sky::POLE..sky::POLE + 3].copy_from_slice(&env[ENV_POLE..ENV_POLE + 3]);
        self.words[sky::MAPPED] = if moon_mapped { 1.0 } else { 0.0 };
    }

    /// Writes `words` to the device. The frame calls it only when its read found them
    /// changed.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        pass.set_pipeline(self.pipeline.id);
        pass.set_bind_group(0, self.group, &[]);
        pass.draw(3, 1, 0, 0);
    }
}

/// Puts into `out` at `at` the three columns by which the ray under NDC
/// (x, y) is `x·col0 + y·col1 + col2`. They are the screen's lens, found from
/// the view's planes as the cast's eye finds it (pxl-spec `Frustum::lens`):
/// its right and its up scaled by their tangents, and its forward. Each is
/// computed in f64 before it is narrowed. The columns are zero where there
/// is no lens, as without a screen.
fn put_ray(out: &mut [f32], at: usize, view_proj: &Mat4) {
    let mut planes = Frustum::default();
    mat4::frustum_planes(&mut planes.planes, view_proj);
    let columns = planes.lens().map_or([[0.0; 3]; 3], |l| {
        [
            l.right.map(|c| c * l.tan[0]),
            l.up.map(|c| c * l.tan[1]),
            l.forward,
        ]
    });
    for (k, column) in columns.iter().enumerate() {
        for (o, e) in out[at + 4 * k..at + 4 * k + 3].iter_mut().zip(column) {
            *o = *e as f32;
        }
    }
}
