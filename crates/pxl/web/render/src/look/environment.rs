// pxl-render/look/environment.rs

//! The lane the frame, sky and atmosphere uniforms are laid out from: sun and
//! moon light taken through the air. The light from everywhere else is the
//! sky's own, which the shaders read from its tables. Directions are
//! in the map's axes (X east, Y north, Z up), as is the sky.

use pxl_spec::space::v3::{dot3, norm3, V3};

use crate::look::atmosphere::{light_at, moon_phase_light, white, MOON_LIGHT, NIGHT_COLOR};

/// Lighting run (24), stars' fade, clock and the map center's longitude (4),
/// air (4), second light (8), sun direction (4), stars' frame (12), moon
/// direction and sky brightness (4).
pub const ENV_FLOATS: usize = 60;
pub const ENV_FOG_NEAR: usize = 7;
pub const ENV_FOG_FAR: usize = 11;
pub const ENV_SKY: usize = 24;
pub const ENV_AIR: usize = 28;
pub const ENV_OTHER_LIGHT: usize = 32;
pub const ENV_SUN: usize = 40;
pub const ENV_STARS: usize = 44;
pub const ENV_MOON: usize = 56;

#[derive(Clone, Copy)]
pub struct Light {
    pub sun: V3,
    pub moon: V3,
    /// The stars' fade in from the first frame, 0 to 1.
    pub stars: f64,
    pub time: f64,
    pub fog_near: f64,
    pub fog_far: f64,
    pub altitude_km: f64,
    pub km_per_u: f64,
    pub latitude: f64,
    pub longitude: f64,
    /// Where the sky draws its own ground, so the terrain does not stand over it.
    pub ground_km: f64,
    /// The equatorial frame to the local one, as rows.
    pub star_frame: [V3; 3],
}

/// Rec. 709, as constant.wgsl's LUMA.
pub(crate) const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

fn luminance(c: [f64; 3]) -> f64 {
    dot3(&LUMA, &c)
}

pub fn write_environment(o: &mut [f32; ENV_FLOATS], light: &Light) {
    // Both lights are summed so the light does not snap from one direction to the
    // other where their intensities cross at dusk; the brighter casts the shadow.
    let sun_dir = norm3(&light.sun);
    let w = white();
    // The light the ground takes, as the ground sees it: from orbit the camera
    // sees the sun over a lower horizon and through no air, which lit the
    // ground white at dawn under the ground's low exposure.
    let sun_t = light_at(light.ground_km, sun_dir[2]);
    let sun_color: [f64; 3] = core::array::from_fn(|k| sun_t[k] * w[k]);
    let moon_dir = norm3(&light.moon);
    // Night as the sky shows it, past which the stars are lost in the day.
    let night = 1.0 - smoothstep(-0.1, 0.0, sun_dir[2]);
    let elongation = libm::acos(dot3(&sun_dir, &moon_dir).clamp(-1.0, 1.0));
    let phase = moon_phase_light(core::f64::consts::PI - elongation);
    let through = light_at(light.ground_km, moon_dir[2]);
    // By day the sun leaves the Moon nothing, so its light and its sky are
    // not worked out at all.
    let moon_color: [f64; 3] = core::array::from_fn(|k| NIGHT_COLOR[k] * MOON_LIGHT * phase * night * through[k] * w[k]);
    let sun = (sun_dir, sun_color);
    let moon = (moon_dir, moon_color);
    let (first, other) = if luminance(moon_color) > luminance(sun_color) { (moon, sun) } else { (sun, moon) };
    for (at, (d, color)) in [(0, first), (ENV_OTHER_LIGHT, other)] {
        put3(o, at, f32s(&d));
        put3(o, at + 4, [color[0] as f32, color[1] as f32, color[2] as f32]);
    }
    o[ENV_FOG_NEAR] = light.fog_near as f32;
    o[ENV_FOG_FAR] = light.fog_far as f32;

    o[ENV_SKY] = (light.stars * night) as f32;
    o[ENV_SKY + 1] = light.time as f32;
    o[ENV_SKY + 2] = light.longitude as f32;

    o[ENV_AIR] = light.altitude_km as f32;
    o[ENV_AIR + 1] = light.km_per_u as f32;
    o[ENV_AIR + 2] = light.latitude as f32;
    o[ENV_AIR + 3] = light.ground_km as f32;
    put3(o, ENV_SUN, f32s(&sun_dir));

    put3(o, ENV_MOON, f32s(&moon_dir));
    o[ENV_MOON + 3] = (MOON_LIGHT * phase * night) as f32;

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
