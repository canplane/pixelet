// pxl-render/look/environment.rs

//! The lane the frame, sky and atmosphere uniforms are laid out from: sun and
//! moon light taken through the air. The light from everywhere else is the
//! sky's own, which the shaders read from its tables. Directions are
//! in the map's axes (X east, Y north, Z up), as is the sky.

use pxl_spec::space::coord::v3::{self, V3};

use crate::look::atmosphere::{moon_phase_light, MOON_LIGHT, NIGHT_COLOR};

/// The sun's light (8): its direction, a pad, its colour, a pad; the
/// Moon's (8): its direction, its light, its colour, a pad; the stars'
/// fade, clock and the anchor's longitude (4); air (4); the stars' frame
/// (12).
pub const ENV_FLOATS: usize = 36;
pub const ENV_SUN_LIGHT: usize = 0;
pub const ENV_MOON_LIGHT: usize = 8;
/// A light's colour, past its direction.
pub const LIGHT_COLOR: usize = 4;
pub const ENV_SKY: usize = 16;
pub const ENV_AIR: usize = 20;
pub const ENV_STARS: usize = 24;

#[derive(Clone, Copy)]
pub struct Light {
    pub sun: V3,
    pub moon: V3,
    /// The stars' fade in from the first frame, 0 to 1.
    pub stars: f64,
    pub time: f64,
    pub altitude_km: f64,
    pub km_per_u: f64,
    pub latitude: f64,
    pub longitude: f64,
    /// Where the sky draws its own ground, so the terrain does not stand over it.
    pub ground_km: f64,
    /// The equatorial frame to the local one, as rows.
    pub star_frame: [V3; 3],
}

pub fn write_environment(o: &mut [f32; ENV_FLOATS], light: &Light) {
    // Both lights, summed so the light does not snap where they cross at
    // dusk, each as it reaches the air: the sun's first, the moon's second,
    // each against a sun overhead at sea level, which the shaders take as
    // white (atmos_lut.wgsl `table_white`).
    let sun_dir = v3::norm(&light.sun);
    let moon_dir = v3::norm(&light.moon);
    // The stars are lost in the day.
    let night = 1.0 - smoothstep(-0.1, 0.0, sun_dir[2]);
    let elongation = libm::acos(v3::dot(&sun_dir, &moon_dir).clamp(-1.0, 1.0));
    let phase = moon_phase_light(core::f64::consts::PI - elongation);
    // Wherever the Moon is up; by day the sun outshines it.
    let moon_color: [f64; 3] = core::array::from_fn(|k| NIGHT_COLOR[k] * MOON_LIGHT * phase);
    for (at, (d, color)) in [(ENV_SUN_LIGHT, (sun_dir, [1.0; 3])), (ENV_MOON_LIGHT, (moon_dir, moon_color))] {
        put3(o, at, f32s(&d));
        put3(o, at + LIGHT_COLOR, [color[0] as f32, color[1] as f32, color[2] as f32]);
    }
    o[ENV_MOON_LIGHT + 3] = (MOON_LIGHT * phase) as f32;
    o[ENV_SKY] = (light.stars * night) as f32;
    o[ENV_SKY + 1] = light.time as f32;
    o[ENV_SKY + 2] = light.longitude as f32;

    o[ENV_AIR] = light.altitude_km as f32;
    o[ENV_AIR + 1] = light.km_per_u as f32;
    o[ENV_AIR + 2] = light.latitude as f32;
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
