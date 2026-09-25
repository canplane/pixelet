// pxl-render/gpu/stars.rs

//! The stars: the Yale Bright Star Catalogue (Hoffleit and Warren, fifth
//! revised edition, 1991; CDS V/50), baked by script/bake-stars.ts into eight
//! bytes a star, and drawn as one instanced strip of four corners a star
//! (frame/shader/stars.wgsl) right after the sky, under everything else.
//!
//! What never changes of a star is taken once, here, when the pass is made:
//! its direction in the equatorial frame, its flux from its magnitude, and
//! its colour from its temperature — Planck's law at three wavelengths,
//! which the vertex stage would otherwise work out for each of a star's four
//! corners every frame. What the frame changes — where the sky has turned
//! to, the air the star is seen through — stays the vertex stage's.
//!
//! What it costs: 72,768 bytes in the module, 291,072 on the device, eight
//! floats a star; a draw of 9,096 quads, most of whose stars are culled in
//! the vertex stage as too faint for the frame's eight bits, and none at all
//! while the stars are faded out, which is every hour of daylight.

use pxl_spec::space::mat4::Mat4;
use pxl_spec::space::v3::V3;

use crate::frame::shader::record::PIPELINE_STARS;
use crate::frame::stage::{stage_lane, STAGE_AT};
use crate::gpu::api::{buffer_entry, buffer_usage, bytes, object, word_bytes, GpuApi};
use crate::gpu::api_spec::format;
use crate::gpu::pipeline::{Built, Pipelines};
use crate::look::atmosphere::transmittance;
use crate::look::environment::{ENV_AIR, ENV_SKY, ENV_STARS};

/// The catalogue as the bake wrote it.
const CATALOGUE: &[u8] = include_bytes!("../frame/shader/bright-stars.bin");
const STAR_BYTES: usize = 8;
/// Floats a star takes on the device: its direction and its flux, then its
/// colour and a word of padding (stars.wgsl's `catalogue`).
const STAR_FLOATS: usize = 8;

/// Floats the Stars uniform (stars.wgsl's Stars) holds.
pub(crate) const STARS_FLOATS: usize = 16 + 16 + 4 + 4 + 4;
pub(crate) const SKY: usize = 16;
pub(crate) const VIEWPORT: usize = 32;
pub(crate) const INTENSITY: usize = 34;
pub(crate) const DEPTH: usize = 36;
pub(crate) const DENSITY: usize = 39;
pub(crate) const UP: usize = 40;

/// Below this the stars have faded out, and the pass draws nothing.
const SHOWN: f32 = 0.001;

pub struct StarPass {
    built: Built,
    uniform: u32,
    group: u32,
    words: [f32; STARS_FLOATS],
}

impl StarPass {
    pub fn new(g: &GpuApi, pipelines: &mut Pipelines) -> StarPass {
        let built = pipelines.build(g, PIPELINE_STARS, format::CANVAS);
        let uniform = object();
        (g.buffer)(uniform, word_bytes(STARS_FLOATS), buffer_usage::UNIFORM | buffer_usage::COPY_DST);
        let stars: Vec<f32> = CATALOGUE.as_chunks::<STAR_BYTES>().0.iter().flat_map(star).collect();
        let catalogue = object();
        (g.buffer)(catalogue, word_bytes(stars.len()), buffer_usage::STORAGE | buffer_usage::COPY_DST);
        (g.write_buffer)(catalogue, 0, bytes(&stars));
        let group = object();
        let mut e = [0u32; 10];
        e[..5].copy_from_slice(&buffer_entry(0, uniform));
        e[5..].copy_from_slice(&buffer_entry(1, catalogue));
        (g.bind_group)(group, built.layouts[0], &e);
        StarPass {
            built,
            uniform,
            group,
            words: [0.0; STARS_FLOATS],
        }
    }

    /// The frame's stars: `view_proj`, the root frame to WebGPU clip,
    /// the environment's lane `env`, the world surface's size in device
    /// pixels, and straight up where the eye stands, which a star's height
    /// and the air it is seen through are measured from. Nothing is written
    /// while they are faded out.
    pub fn lay_out(&mut self, g: &GpuApi, view_proj: &Mat4, env: &[f32], width: u32, height: u32, up: &V3) {
        let u = &mut self.words;
        u[INTENSITY] = env[ENV_SKY];
        if u[INTENSITY] <= SHOWN {
            return;
        }
        for (k, v) in view_proj.iter().enumerate() {
            u[k] = *v as f32;
        }
        u[SKY..SKY + 12].copy_from_slice(&env[ENV_STARS..ENV_STARS + 12]);
        u[SKY + 12..SKY + 16].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]);
        u[VIEWPORT] = width as f32;
        u[VIEWPORT + 1] = height as f32;
        let css = stage_lane()[STAGE_AT::VIEWPORT];
        u[DENSITY] = if css > 0.0 { (width as f64 / css) as f32 } else { 1.0 };
        for (w, v) in u[UP..UP + 3].iter_mut().zip(up) {
            *w = *v as f32;
        }
        // The air overhead from where the camera stands: from orbit a star
        // crosses none of it.
        let t = transmittance(env[ENV_AIR] as f64, 1.0);
        for k in 0..3 {
            u[DEPTH + k] = -(t[k].max(1e-6)).ln() as f32;
        }
        (g.write_buffer)(self.uniform, 0, bytes(u));
    }

    /// Every star into the open pass, where they show.
    pub fn draw(&self, g: &GpuApi, pass: u32) {
        if self.words[INTENSITY] <= SHOWN {
            return;
        }
        (g.set_pipeline)(pass, self.built.pipeline);
        (g.set_bind_group)(pass, 0, self.group, &[]);
        (g.draw)(pass, 4, (CATALOGUE.len() / STAR_BYTES) as u32, 0, 0);
    }
}

/// A star as the bake packed it — its right ascension as a u16 of a turn,
/// its declination as an i16 of a quarter turn, its visual magnitude and its
/// B−V colour index as i16 thousandths (script/bake-stars.ts) — as the
/// vertex stage reads it.
fn star(packed: &[u8; STAR_BYTES]) -> [f32; STAR_FLOATS] {
    let word = |k: usize| [packed[2 * k], packed[2 * k + 1]];
    let ra = u16::from_le_bytes(word(0)) as f64 / 65536.0 * core::f64::consts::TAU;
    let dec = i16::from_le_bytes(word(1)) as f64 / 32767.0 * core::f64::consts::FRAC_PI_2;
    let vmag = i16::from_le_bytes(word(2)) as f64 / 1000.0;
    let bv = i16::from_le_bytes(word(3)) as f64 / 1000.0;
    let flux = 10f64.powf(-0.4 * vmag);
    let [r, g, b] = star_colour(bv);
    [
        (dec.cos() * ra.cos()) as f32,
        (dec.cos() * ra.sin()) as f32,
        dec.sin() as f32,
        flux as f32,
        r as f32,
        g as f32,
        b as f32,
        0.0,
    ]
}

/// Planck's law at a wavelength `lambda` (nm) and a temperature `t` (K), up
/// to a constant the colour divides out.
fn planck(lambda: f64, t: f64) -> f64 {
    let l = lambda * 1e-9;
    1.0 / (l.powi(5) * ((1.4388e-2 / (l * t)).exp() - 1.0))
}

/// B−V to temperature (Ballesteros 2012), and the temperature to a colour at
/// the three primaries' wavelengths relative to a 6,500 K white, normalized
/// to its brightest channel and taken half-way to white: the eye's cone
/// response at night leaves a star's colour a tint.
fn star_colour(bv: f64) -> [f64; 3] {
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let c = [610.0, 550.0, 465.0].map(|lambda| planck(lambda, t) / planck(lambda, 6500.0));
    let peak = c[0].max(c[1]).max(c[2]);
    c.map(|v| 0.5 + 0.5 * v / peak)
}

#[cfg(test)]
#[path = "stars.test.rs"]
mod test;
