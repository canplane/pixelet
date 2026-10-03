// pxl-render/gpu/stars.rs

//! The Yale Bright Star Catalog (Hoffleit and Warren 1991, CDS V/50), drawn
//! after the sky as one instanced strip. Direction, flux and color are computed
//! once here rather than per corner per frame. Nothing is drawn in daylight.

use pxl_spec::space::mat4::Mat4;

use crate::frame::shader::record::PIPELINE_STARS;
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, word_bytes, Gpu};
use crate::gpu::api_spec::format;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::gpu::renderer::put_mat;
use crate::look::atmosphere::transmittance;
use crate::look::environment::{ENV_AIR, ENV_SKY, ENV_STARS};

const CATALOG: &[u8] = include_bytes!("../frame/shader/bright-stars.bin");
const STAR_BYTES: usize = 8;
/// Direction and flux, then color and padding.
const STAR_FLOATS: usize = 8;

pub(crate) const STARS_FLOATS: usize = 16 + 16 + 4 + 4;
pub(crate) const SKY: usize = 16;
pub(crate) const DEPTH: usize = 32;
pub(crate) const DENSITY: usize = 35;
pub(crate) const INTENSITY: usize = 36;

const SHOWN: f32 = 0.001;

pub struct StarPass {
    built: Built,
    frame_group: u32,
    uniform: u32,
    group: u32,
    words: [f32; STARS_FLOATS],
}

impl StarPass {
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, frame_buffer: u32) -> StarPass {
        let built = pipelines.build(g, PIPELINE_STARS, format::CANVAS);
        let uniform = object();
        g.buffer(uniform, word_bytes(STARS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let stars: Vec<f32> = CATALOG.as_chunks::<STAR_BYTES>().0.iter().flat_map(star).collect();
        let catalog = object();
        g.buffer(catalog, word_bytes(stars.len()), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(catalog, 0, bytes(&stars));
        let frame_group = object();
        g.bind_group(frame_group, built.layouts[0], &buffer_entry(0, frame_buffer));
        let group = object();
        let mut e = [0u32; 10];
        e[..5].copy_from_slice(&buffer_entry(0, uniform));
        e[5..].copy_from_slice(&buffer_entry(1, catalog));
        g.bind_group(group, built.layouts[1], &e);
        StarPass {
            built,
            frame_group,
            uniform,
            group,
            words: [0.0; STARS_FLOATS],
        }
    }

    /// Nothing is written while the stars are faded out.
    pub fn lay_out(&mut self, g: &Gpu, view_proj: &Mat4, env: &[f32], width: u32) {
        let u = &mut self.words;
        u[INTENSITY] = env[ENV_SKY];
        if u[INTENSITY] <= SHOWN {
            return;
        }
        put_mat(u, 0, view_proj);
        u[SKY..SKY + 12].copy_from_slice(&env[ENV_STARS..ENV_STARS + 12]);
        u[SKY + 12..SKY + 16].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
        let css = stage_lane()[STAGE_AT::VIEWPORT];
        u[DENSITY] = if css > 0.0 { (width as f64 / css) as f32 } else { 1.0 };
        // From orbit a star crosses none of the air.
        let t = transmittance(env[ENV_AIR] as f64, 1.0);
        for k in 0..3 {
            u[DEPTH + k] = -libm::log(t[k].max(1e-6)) as f32;
        }
        g.write_buffer(self.uniform, 0, bytes(u));
    }

    pub fn draw(&self, g: &Gpu, pass: u32) {
        if self.words[INTENSITY] <= SHOWN {
            return;
        }
        g.set_pipeline(pass, self.built.pipeline);
        g.set_bind_group(pass, 0, self.frame_group, &[]);
        g.set_bind_group(pass, 1, self.group, &[]);
        g.draw(pass, 4, (CATALOG.len() / STAR_BYTES) as u32, 0, 0);
    }
}

/// RA as a u16 of a turn, declination as an i16 of a quarter turn, magnitude
/// and B−V as i16 thousandths.
fn star(packed: &[u8; STAR_BYTES]) -> [f32; STAR_FLOATS] {
    let word = |k: usize| [packed[2 * k], packed[2 * k + 1]];
    let ra = u16::from_le_bytes(word(0)) as f64 / 65536.0 * core::f64::consts::TAU;
    let dec = i16::from_le_bytes(word(1)) as f64 / 32767.0 * core::f64::consts::FRAC_PI_2;
    let vmag = i16::from_le_bytes(word(2)) as f64 / 1000.0;
    let bv = i16::from_le_bytes(word(3)) as f64 / 1000.0;
    let flux = libm::pow(10f64, -0.4 * vmag);
    let [r, g, b] = star_color(bv);
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
    1.0 / (l.powi(5) * (libm::exp(1.4388e-2 / (l * t)) - 1.0))
}

/// Ballesteros 2012 B−V to temperature, relative to 6,500 K white, taken
/// half-way to white: night cone response leaves a star only a tint.
fn star_color(bv: f64) -> [f64; 3] {
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let c = [610.0, 550.0, 465.0].map(|lambda| planck(lambda, t) / planck(lambda, 6500.0));
    let peak = c[0].max(c[1]).max(c[2]);
    c.map(|v| 0.5 + 0.5 * v / peak)
}

#[cfg(test)]
#[path = "stars.test.rs"]
mod test;
