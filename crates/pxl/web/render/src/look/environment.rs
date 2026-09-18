// pxl-render/look/environment.rs

//! The light, the fog and the sky as the frame is lit: a theme's floats as the
//! kernel eased them and the sun and the moon where it placed them
//! (crates/pxl/web/kernel/src/layer/environment/state.rs), composed into the lane the
//! renderer's frame and sky uniforms are laid out from (gpu/renderer.rs).
//!
//! The terrain's light is the true, unturned direction: the world turns
//! under a fixed camera by the azimuth, and a light turned with it would
//! cancel the turn out. The sky is drawn in the camera's frame, where the
//! sun has to turn with the world to stay over the ground it lights.

/// Floats a theme takes, linear colours: fog (3), sky top, middle and
/// horizon (9), sun glow (3), ambient colour and intensity (4), hemisphere
/// sky and ground and intensity (7), sun colour and intensity (4), night
/// (1), fog near and far (2), the lighting floor (1).
pub const THEME_FLOATS: usize = 34;

/// Where the colours of a theme sit in its floats.
const FOG: usize = 0;
const SKY_TOP: usize = 3;
const SKY_MIDDLE: usize = 6;
const SKY_HORIZON: usize = 9;
const SUN_GLOW: usize = 12;
const AMBIENT: usize = 15;
const HEMI_SKY: usize = 19;
const HEMI_GROUND: usize = 22;
const HEMI_INTENSITY: usize = 25;
const SUN: usize = 26;
pub const NIGHT: usize = 30;
const FOG_NEAR: usize = 31;
const FOG_FAR: usize = 32;
const FLOOR: usize = 33;

/// Floats the environment's lane takes: the frame uniform's lighting run (24)
/// and the sky uniform's colours and directions (24), each aligned to the
/// renderer's own layout from its twentieth float; then the second light (8):
/// the dimmer of the sun and the moon, its direction and its colour.
pub const ENV_FLOATS: usize = 56;
/// Where the second light starts in the environment's lane.
pub const ENV_OTHER_LIGHT: usize = 48;

/// The moon's light: a fixed colour (#9fb4d6, linear), brighter than the
/// night theme's sun so that at night it is the light the terrain takes.
const MOON_COLOR: [f32; 3] = [0.3467, 0.4564, 0.6724];

/// What the lane is composed from.
#[derive(Clone, Copy)]
pub struct Light {
    /// The theme as eased so far, THEME_FLOATS linear floats.
    pub theme: [f32; THEME_FLOATS],
    /// The sun's and the moon's directions, unturned.
    pub sun: [f64; 3],
    pub moon: [f64; 3],
    pub moon_intensity: f64,
    /// How far the stars have faded in, from 0 to 1.
    pub stars: f64,
    /// The sky's clock, in seconds: the clouds drift and the stars twinkle by
    /// it.
    pub time: f64,
    /// The map's azimuth: the root frame's turn about Z.
    pub azimuth: f64,
}

/// The lane `light` lights a frame with.
pub fn write_environment(o: &mut [f32; ENV_FLOATS], light: &Light) {
    let c = &light.theme;
    // The terrain is lit by both the sun and the moon, summed: the
    // brighter is the frame's sun, which casts the shadow, and the other
    // is the second light, unshadowed. Summed rather than the brighter
    // alone, so the light does not snap from one direction to the other
    // where their intensities cross during a dusk or dawn ease.
    let sun_i = c[SUN + 3] as f64;
    let sun = (light.sun, [c[SUN], c[SUN + 1], c[SUN + 2]], sun_i);
    let moon = (light.moon, MOON_COLOR, light.moon_intensity);
    let (first, other) = if light.moon_intensity > sun_i { (moon, sun) } else { (sun, moon) };
    for (at, (dir, color, intensity)) in [(0, first), (ENV_OTHER_LIGHT, other)] {
        let d = normalize(dir);
        put3(o, at, [d[0] as f32, d[1] as f32, d[2] as f32]);
        put3(o, at + 4, [color[0] * intensity as f32, color[1] * intensity as f32, color[2] * intensity as f32]);
    }
    o[7] = c[FOG_NEAR];
    let ambient_i = c[AMBIENT + 3];
    put3(o, 8, [c[AMBIENT] * ambient_i, c[AMBIENT + 1] * ambient_i, c[AMBIENT + 2] * ambient_i]);
    o[11] = c[FOG_FAR];
    let hemi_i = c[HEMI_INTENSITY];
    put3(o, 12, [c[HEMI_SKY] * hemi_i, c[HEMI_SKY + 1] * hemi_i, c[HEMI_SKY + 2] * hemi_i]);
    o[15] = c[FLOOR];
    put3(o, 16, [c[HEMI_GROUND] * hemi_i, c[HEMI_GROUND + 1] * hemi_i, c[HEMI_GROUND + 2] * hemi_i]);
    put3(o, 20, [c[FOG], c[FOG + 1], c[FOG + 2]]);

    // The sky, turned by the map's azimuth.
    let az = light.azimuth;
    put3(o, 24, [c[SKY_TOP], c[SKY_TOP + 1], c[SKY_TOP + 2]]);
    o[27] = light.time as f32;
    put3(o, 28, [c[SKY_MIDDLE], c[SKY_MIDDLE + 1], c[SKY_MIDDLE + 2]]);
    o[31] = light.stars as f32;
    put3(o, 32, [c[SKY_HORIZON], c[SKY_HORIZON + 1], c[SKY_HORIZON + 2]]);
    o[35] = az as f32;
    put3(o, 36, turned(normalize(light.sun), az));
    put3(o, 40, [c[SUN_GLOW], c[SUN_GLOW + 1], c[SUN_GLOW + 2]]);
    put3(o, 44, turned(normalize(light.moon), az));
}

fn put3(o: &mut [f32; ENV_FLOATS], at: usize, v: [f32; 3]) {
    o[at..at + 3].copy_from_slice(&v);
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0.0 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn turned(v: [f64; 3], az: f64) -> [f32; 3] {
    let (s, c) = az.sin_cos();
    [(v[0] * c - v[1] * s) as f32, (v[0] * s + v[1] * c) as f32, v[2] as f32]
}
