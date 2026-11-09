// pxl-render/pass/star.rs

//! Draws the Yale Bright Star Catalog (Hoffleit and Warren 1991, CDS V/50) after the sky, as
//! one instanced strip. Nothing is drawn in daylight. The catalog's stars are the whole night
//! sky.

use pxl_spec::asset::star::{self, RECORD_BYTES};
use pxl_spec::space::coord::mat4::Mat4;

use crate::frame::shader::record::PIPELINE_STAR;
use crate::frame::shader::word::stars::{DENSITY, INTENSITY, SKY, VIEW_PROJ};
use crate::frame::shader::word::{STARS_FLOATS, STAR_RECORD_FLOATS};
use crate::gpu::api::{buf_entry, buffer_usage, bytes, word_bytes, Entry, Gpu, TrackedRenderPass};
use crate::gpu::pipeline::{Pipeline, PipelineCache};
use crate::gpu::put::put_mat;
use crate::look::environment;
use crate::param::sky::star::DRAWN_ABOVE;
use crate::pass::atmosphere::AIR_ENTRIES;

const CATALOG: &[u8] = include_bytes!("bright_star.bin");

pub struct StarPass {
    pipeline: Pipeline,
    frame_group: u32,
    uniform: u32,
    group: u32,
    words: [f32; STARS_FLOATS],
}

impl StarPass {
    /// `air` holds the air's entries (pass/atmosphere.rs `entries`), so a star is dimmed by the
    /// air above the eye as the air's table holds it.
    pub fn new(
        g: &Gpu,
        pipelines: &mut PipelineCache,
        frame_buf: u32,
        air: [Entry; AIR_ENTRIES],
    ) -> StarPass {
        let pipeline = pipelines.build_world(g, PIPELINE_STAR);
        let uniform = g.buf(
            word_bytes(STARS_FLOATS),
            buffer_usage::UNIFORM | buffer_usage::COPY_DST,
        );
        let stars: Vec<f32> = CATALOG
            .as_chunks::<RECORD_BYTES>()
            .0
            .iter()
            .flat_map(star_floats)
            .collect();
        let catalog = g.buf(
            word_bytes(stars.len()),
            buffer_usage::STORAGE | buffer_usage::COPY_DST,
        );
        g.write_buf(catalog, 0, bytes(&stars));
        // Only the air's entries that the layout binds go into the group.
        let layout = pipeline.layouts[0];
        let mut frame = vec![buf_entry(0, frame_buf)];
        frame.extend(air.iter().filter(|e| layout.binds(e.binding)));
        let frame_group = g.bind_group(layout, &frame);
        let group = g.bind_group(
            pipeline.layouts[1],
            &[buf_entry(0, uniform), buf_entry(1, catalog)],
        );
        StarPass {
            pipeline,
            frame_group,
            uniform,
            group,
            words: [0.0; STARS_FLOATS],
        }
    }

    /// The words the stars are drawn by, as `prepare` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// Lays out the stars' words. While the stars are faded out, only the intensity is
    /// written. `density` is the drawn pixels a CSS pixel spans (`Surface::px_per_css`).
    pub fn prepare(&mut self, view_proj: &Mat4, env: &[f32], density: f64) {
        let u = &mut self.words;
        u[INTENSITY] = env[environment::SKY];
        if u[INTENSITY] <= DRAWN_ABOVE {
            return;
        }
        put_mat(u, VIEW_PROJ, view_proj);
        u[SKY..SKY + environment::STAR_FRAME_FLOATS].copy_from_slice(
            &env[environment::STARS..environment::STARS + environment::STAR_FRAME_FLOATS],
        );
        u[SKY + 12..SKY + 16].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
        u[DENSITY] = density as f32;
    }

    /// Writes `words` to the device. The frame calls it only when its read found them
    /// changed.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        if self.words[INTENSITY] <= DRAWN_ABOVE {
            return;
        }
        pass.set_pipeline(self.pipeline.id);
        pass.set_bind_group(0, self.frame_group, &[]);
        pass.set_bind_group(1, self.group, &[]);
        pass.draw(4, (CATALOG.len() / RECORD_BYTES) as u32, 0, 0);
    }
}

/// A catalog record's star as the sky draws it: its direction, its flux and its color. The
/// record's layout is the bake's, read by pxl-spec asset/star.rs `star`.
fn star_floats(packed: &[u8; RECORD_BYTES]) -> [f32; STAR_RECORD_FLOATS] {
    let s = star::star(packed);
    let (ra, dec) = (s.ra.to_radians(), s.dec.to_radians());
    let flux = libm::pow(10f64, -0.4 * s.vmag);
    let [r, g, b] = star_color(s.bv);
    [
        (libm::cos(dec) * libm::cos(ra)) as f32,
        (libm::cos(dec) * libm::sin(ra)) as f32,
        libm::sin(dec) as f32,
        flux as f32,
        r as f32,
        g as f32,
        b as f32,
        0.0,
    ]
}

/// Planck's law at `lambda` nm and `t` K, up to a constant factor, which dividing one color by
/// another cancels.
fn planck(lambda: f64, t: f64) -> f64 {
    let l = lambda * 1e-9;
    1.0 / ((l * l) * (l * l) * l * (libm::exp(1.4388e-2 / (l * t)) - 1.0))
}

/// A star's color from its B−V index: its temperature by Ballesteros 2012, relative to 6,500 K
/// white, then taken half-way to white, as the eye's response at night leaves a star only a
/// tint.
fn star_color(bv: f64) -> [f64; 3] {
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let c = [610.0, 550.0, 465.0].map(|lambda| planck(lambda, t) / planck(lambda, 6500.0));
    let peak = c[0].max(c[1]).max(c[2]);
    c.map(|v| 0.5 + 0.5 * v / peak)
}
