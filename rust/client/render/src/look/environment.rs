// pxl-render/look/environment.rs

//! The lane the frame, sky and atmosphere uniforms are laid out from: sun and
//! moon light taken through the air. The light from everywhere else is the
//! sky's own, which the shaders read from its tables. Directions are
//! in the map's axes (X east, Y north, Z up), as is the sky.

use pxl_spec::space::coord::v3::{self, V3};

use crate::look::atmosphere::{moon_phase_light, MOON_LIGHT, NIGHT_COLOR};
use crate::param::night;

/// The sun's direction and a pad (4); the moon's direction and its light
/// (4); the moon's color and a pad (4), the sun's being white; the stars'
/// fade, clock and the anchor's longitude (4); air (4); the stars' frame
/// (12).
pub const ENV_FLOATS: usize = ENV_STARS + ENV_STAR_FRAME_FLOATS;
pub const ENV_SUN: usize = 0;
pub const ENV_MOON: usize = 4;
pub const ENV_MOON_COLOR: usize = 8;
pub const ENV_SKY: usize = 12;
pub const ENV_AIR: usize = 16;
pub const ENV_STARS: usize = 20;
/// The stars' frame, three columns a vec4 each, their `w` unread.
pub const ENV_STAR_FRAME_FLOATS: usize = 3 * 4;
/// Its third column, the celestial pole.
pub const ENV_POLE: usize = ENV_STARS + 2 * 4;

#[derive(Clone, Copy)]
pub struct Light {
    pub sun: V3,
    pub moon: V3,
    /// The stars' fade in from the first frame, 0 to 1.
    pub stars: f64,
    pub time: f64,
    pub alt_km: f64,
    pub km_per_u: f64,
    pub lat: f64,
    pub lon: f64,
    /// Where the sky draws its own ground, so the terrain does not stand over it.
    pub ground_km: f64,
    /// The equatorial frame to the local one, as rows.
    pub star_frame: [V3; 3],
}

pub fn write_environment(o: &mut [f32; ENV_FLOATS], light: &Light) {
    // Both lights, summed so the light does not snap where they cross at
    // dusk, each as it reaches the air, against a sun overhead at sea
    // level, which the shaders take as white (atmos_lut.wgsl
    // `table_white`): the sun's is that white, the moon's its color.
    let sun_dir = v3::norm(&light.sun);
    let moon_dir = v3::norm(&light.moon);
    // The stars are lost in the day.
    let [from, to] = night::FALL.map(f64::from);
    let night = 1.0 - smoothstep(from, to, sun_dir[2]);
    let elongation = libm::acos(v3::dot(&sun_dir, &moon_dir).clamp(-1.0, 1.0));
    let phase = moon_phase_light(core::f64::consts::PI - elongation);
    // Wherever the moon is up; by day the sun outshines it.
    let moon_color: [f64; 3] = core::array::from_fn(|k| NIGHT_COLOR[k] * MOON_LIGHT * phase);
    put3(o, ENV_SUN, f32s(&sun_dir));
    put3(o, ENV_MOON, f32s(&moon_dir));
    o[ENV_MOON + 3] = (MOON_LIGHT * phase) as f32;
    put3(o, ENV_MOON_COLOR, f32s(&moon_color));
    o[ENV_SKY] = (light.stars * night) as f32;
    o[ENV_SKY + 1] = light.time as f32;
    o[ENV_SKY + 2] = light.lon as f32;

    o[ENV_AIR] = light.alt_km as f32;
    o[ENV_AIR + 1] = light.km_per_u as f32;
    o[ENV_AIR + 2] = light.lat as f32;
    o[ENV_AIR + 3] = light.ground_km as f32;

    let columns: [V3; 3] = core::array::from_fn(|k| light.star_frame.map(|row| row[k]));
    for (k, column) in columns.iter().enumerate() {
        put3(o, ENV_STARS + 4 * k, f32s(column));
    }
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn put3(o: &mut [f32; ENV_FLOATS], at: usize, v: [f32; 3]) {
    o[at..at + 3].copy_from_slice(&v);
}

fn f32s(v: &V3) -> [f32; 3] {
    v.map(|e| e as f32)
}
