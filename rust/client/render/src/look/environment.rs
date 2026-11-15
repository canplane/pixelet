// pxl-render/look/environment.rs

//! The environment lane, from which the frame, sky and atmosphere uniforms are laid out: the
//! sun's and the moon's light as they reach the air. Light from everywhere else is the sky's
//! own, which the shaders read from its tables. Directions are in the map's axes (X east,
//! Y north, Z up), as the sky's are.

use pxl_content::param::night;
use pxl_spec::space::coord::v3::{self, V3};

use crate::look::atmosphere::{MOON_LIGHT, NIGHT_COLOR, moon_phase_light};

/// The lane's floats, a vec4 each unless said: the sun's direction and a pad; the moon's
/// direction and its light; the moon's color and a pad, the sun's color being white; the
/// stars' fade and three pads; the air; and the stars' frame, three vec4s.
pub const FLOATS: usize = STARS + STAR_FRAME_FLOATS;
pub const SUN: usize = 0;
pub const MOON: usize = 4;
pub const MOON_COLOR: usize = 8;
pub const SKY: usize = 12;
pub const AIR: usize = 16;
pub const STARS: usize = 20;
/// The stars' frame, three columns a vec4 each, their `w` unread.
pub const STAR_FRAME_FLOATS: usize = 3 * 4;
/// The frame's third column, which is the celestial pole.
pub const POLE: usize = STARS + 2 * 4;

#[derive(Clone, Copy)]
pub struct Light {
    pub sun: V3,
    pub moon: V3,
    /// The stars' fade in from the first frame, 0 to 1.
    pub stars: f64,
    pub alt_km: f64,
    pub km_per_u: f64,
    /// Where the sky draws its own ground.
    pub ground_km: f64,
    /// The equatorial frame to the local one, as rows.
    pub star_frame: [V3; 3],
}

pub fn write_environment(o: &mut [f32; FLOATS], light: &Light) {
    // Both lights are summed, so the light does not snap where they cross at dusk. Each is
    // written as it reaches the air, relative to a sun overhead at sea level, which the shaders
    // take as white (atmos_lut.wgsl `table_white`). The sun's light is that white, and the
    // moon's is its color.
    let sun_dir = v3::norm(&light.sun);
    let moon_dir = v3::norm(&light.moon);
    // The stars are lost in the day.
    let [from, to] = night::FALL.map(f64::from);
    let night = 1.0 - smoothstep(from, to, sun_dir[2]);
    let elongation = libm::acos(v3::dot(&sun_dir, &moon_dir).clamp(-1.0, 1.0));
    let phase = moon_phase_light(core::f64::consts::PI - elongation);
    // The moon's light is not faded by day, as the sun outshines it.
    let moon_color: [f64; 3] = core::array::from_fn(|k| NIGHT_COLOR[k] * MOON_LIGHT * phase);
    put3(o, SUN, f32s(&sun_dir));
    put3(o, MOON, f32s(&moon_dir));
    o[MOON + 3] = (MOON_LIGHT * phase) as f32;
    put3(o, MOON_COLOR, f32s(&moon_color));
    o[SKY] = (light.stars * night) as f32;

    o[AIR] = light.alt_km as f32;
    o[AIR + 1] = light.km_per_u as f32;
    o[AIR + 2] = light.ground_km as f32;

    let columns: [V3; 3] = core::array::from_fn(|k| light.star_frame.map(|row| row[k]));
    for (k, column) in columns.iter().enumerate() {
        put3(o, STARS + 4 * k, f32s(column));
    }
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn put3(o: &mut [f32; FLOATS], at: usize, v: [f32; 3]) {
    o[at..at + 3].copy_from_slice(&v);
}

fn f32s(v: &V3) -> [f32; 3] {
    v.map(|e| e as f32)
}
