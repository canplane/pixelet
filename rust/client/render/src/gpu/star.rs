// pxl-render/gpu/star.rs

//! The Yale Bright Star Catalog (Hoffleit and Warren 1991, CDS V/50), drawn
//! after the sky as one instanced strip. Direction, flux and color are computed
//! once here rather than per corner per frame. Nothing is drawn in daylight.
//! The catalog's stars are the whole night sky: no Milky Way and no planets.

use pxl_spec::asset::star::{self, RECORD_BYTES};
use pxl_spec::space::coord::mat4::Mat4;

use crate::frame::shader::record::PIPELINE_STAR;
use crate::frame::shader::word::stars::{DENSITY, INTENSITY, SKY, VIEW_PROJ};
use crate::frame::shader::word::STARS_FLOATS;
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::api::{buf_entry, buffer_usage, bytes, word_bytes, Entry, Gpu, TrackedRenderPass};
use crate::gpu::atmosphere::{AIR_ENTRIES, MEDIUM};
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::put::put_mat;
use crate::gpu::surface::WORLD_COLOR;
use crate::look::environment::{ENV_SKY, ENV_STARS, ENV_STAR_FRAME_FLOATS};
use crate::param::sky::star::DRAWN_ABOVE;

const CATALOG: &[u8] = include_bytes!("../frame/shader/bright-star.bin");
/// Direction and flux, then color and padding.
const STAR_FLOATS: usize = 8;

pub struct StarPass {
    built: Built,
    frame_group: u32,
    uniform: u32,
    group: u32,
    words: [f32; STARS_FLOATS],
}

impl StarPass {
    /// `air`, the air's entries (gpu/atmosphere.rs `entries`): a star is
    /// dimmed by the air above the eye as its table holds it.
    pub fn new(
        g: &Gpu,
        pipelines: &mut Pipelines,
        frame_buf: u32,
        air: [Entry; AIR_ENTRIES],
    ) -> StarPass {
        let built = pipelines.build(g, PIPELINE_STAR, WORLD_COLOR);
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
        let mut frame = vec![buf_entry(0, frame_buf)];
        frame.extend(air.iter().filter(|e| MEDIUM.contains(&e.binding)));
        let frame_group = g.bind_group(built.layouts[0], &frame);
        let group = g.bind_group(
            built.layouts[1],
            &[buf_entry(0, uniform), buf_entry(1, catalog)],
        );
        StarPass {
            built,
            frame_group,
            uniform,
            group,
            words: [0.0; STARS_FLOATS],
        }
    }

    /// The words they are drawn by, as `lay_out` wrote them.
    pub fn words(&self) -> &[f32] {
        &self.words
    }

    /// Nothing is laid out past the intensity while the stars are faded out.
    pub fn lay_out(&mut self, view_proj: &Mat4, env: &[f32], width: u32) {
        let u = &mut self.words;
        u[INTENSITY] = env[ENV_SKY];
        if u[INTENSITY] <= DRAWN_ABOVE {
            return;
        }
        put_mat(u, VIEW_PROJ, view_proj);
        u[SKY..SKY + 12].copy_from_slice(&env[ENV_STARS..ENV_STARS + 12]);
        u[SKY + 12..SKY + 16].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
        let css = stage_lane()[STAGE_AT::VIEWPORT_WIDTH];
        u[DENSITY] = if css > 0.0 {
            (width as f64 / css) as f32
        } else {
            1.0
        };
    }

    /// `words` written, where the frame's read found them moved.
    pub fn write(&self, g: &Gpu) {
        g.write_buf(self.uniform, 0, bytes(&self.words));
    }

    pub fn draw(&self, pass: &mut TrackedRenderPass) {
        if self.words[INTENSITY] <= DRAWN_ABOVE {
            return;
        }
        pass.set_pipeline(self.built.pipeline);
        pass.set_bind_group(0, self.frame_group, &[]);
        pass.set_bind_group(1, self.group, &[]);
        pass.draw(4, (CATALOG.len() / RECORD_BYTES) as u32, 0, 0);
    }
}

/// A record's star (pxl-spec asset/star.rs `star`, the bake's one layout)
/// as the sky draws it: its direction, its flux, its color.
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

/// Up to a constant the color divides out.
fn planck(lambda: f64, t: f64) -> f64 {
    let l = lambda * 1e-9;
    1.0 / ((l * l) * (l * l) * l * (libm::exp(1.4388e-2 / (l * t)) - 1.0))
}

/// Ballesteros 2012 B−V to temperature, relative to 6,500 K white, taken
/// half-way to white: night cone response leaves a star only a tint.
fn star_color(bv: f64) -> [f64; 3] {
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let c = [610.0, 550.0, 465.0].map(|lambda| planck(lambda, t) / planck(lambda, 6500.0));
    let peak = c[0].max(c[1]).max(c[2]);
    c.map(|v| 0.5 + 0.5 * v / peak)
}
