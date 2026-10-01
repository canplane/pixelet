// pxl-render/look/environment.rs

//! The light, the fog and the sky as the frame is lit: a theme's floats as the
//! kernel eased them and the sun and the moon where it placed them
//! (crates/pxl/web/kernel/src/layer/environment/state.rs), composed into the lane the
//! renderer's frame, sky and atmosphere uniforms are laid out from
//! (gpu/renderer.rs).
//!
//! The sun's light is the air's (look/atmosphere.rs): what is left of it at
//! the camera's altitude, which reddens it as it sets and takes it away
//! below the horizon. The sky is the air's too, by day and by night: the
//! moon lights it as the sun does, from its own table, at the brightness a
//! moonlit sky reads at, and with no moon up the night is dark but for the
//! stars. The light from below is the air's too: the land under a face, lit
//! by the same sky and sun (atmos_view.wgsl's ground_bounce). What a theme
//! still gives is its ambient and whether it is night.
//!
//! Every direction here is in the map's own axes, X east, Y north, Z up —
//! the root frame the meshes are placed in. The sky, the stars and the
//! clouds are drawn in that frame too, so nothing is turned by the map's
//! azimuth: the view's matrix turns them with the world.

use pxl_spec::space::v3::{dot3, norm3, V3};

use crate::look::atmosphere::{moon_phase_light, sun_light, white, MOONLIT_SKY};

/// Floats a theme takes, linear colors: ambient color and intensity (4),
/// night (1). What is eased between themes and nothing else: the fog's band
/// and the lighting floor are the same under every theme, and come with the
/// light; the hemisphere is the air's at both ends now, its sky the sky's own
/// light and its ground what that light and the sun leave on the land
/// (atmos_view.wgsl's sky_fill and ground_bounce).
pub const THEME_FLOATS: usize = 5;

/// Where the colors of a theme sit in its floats.
const AMBIENT: usize = 0;
pub const NIGHT: usize = 4;

/// Floats the environment's lane takes: the frame uniform's lighting run
/// (24), laid out as the frame uniform has it; the stars' fade and the sky's
/// clock (4); the air's (4): the camera's altitude and a unit's length, in
/// kilometers, the map center's latitude, and the ground's height under it;
/// the second light (8), the dimmer of the sun and the moon, its direction
/// and its color; the sun's
/// direction (4), which the air is lit from whichever light is the brighter;
/// the stars' frame (12), the equatorial frame to the map's as three columns
/// of four; and the moon's direction and how bright the sky it lights is
/// shown (4).
pub const ENV_FLOATS: usize = 60;
/// Where the sky's run starts in the environment's lane: the stars' fade,
/// then the sky's clock.
pub const ENV_SKY: usize = 24;
/// Where the air's run starts in the environment's lane.
pub const ENV_AIR: usize = 28;
/// Where the second light starts in the environment's lane.
pub const ENV_OTHER_LIGHT: usize = 32;
/// Where the sun's direction starts in the environment's lane.
pub const ENV_SUN: usize = 40;
/// Where the stars' frame starts in the environment's lane.
pub const ENV_STARS: usize = 44;
/// Where the moon's direction and its sky's brightness start.
pub const ENV_MOON: usize = 56;

/// The moon's light on the terrain: a fixed color (#9fb4d6, linear), so
/// that at night it is the light the terrain takes.
const MOON_COLOR: [f32; 3] = [0.3467, 0.4564, 0.6724];

/// What the lane is composed from.
#[derive(Clone, Copy)]
pub struct Light {
    /// The theme as eased so far, THEME_FLOATS linear floats.
    pub theme: [f32; THEME_FLOATS],
    /// The sun's and the moon's directions, in the world's axes.
    pub sun: V3,
    pub moon: V3,
    pub moon_intensity: f64,
    /// How far the stars have faded in, from 0 to 1.
    pub stars: f64,
    /// The sky's clock, in seconds: the clouds drift by it.
    pub time: f64,
    /// The band that hides the world's drawn edge, from where it starts to
    /// where the ground is hidden whole, as far as the view reaches now.
    pub fog_near: f64,
    pub fog_far: f64,
    /// The light every lit face takes whatever it faces, which keeps a
    /// phone's night from going black.
    pub floor: f64,
    /// The camera's altitude above the datum, and a world unit's length, in
    /// kilometers; and the map center's latitude, in radians.
    pub altitude_km: f64,
    pub km_per_u: f64,
    pub latitude: f64,
    /// How high the ground under the map center stands over that datum, in
    /// kilometers: where the sky draws its own ground, so the terrain does
    /// not stand over it (atmos_medium.wgsl's land_altitude_km).
    pub ground_km: f64,
    /// The equatorial frame to the local one, X east, Y north, Z up, as rows
    /// (the kernel's celestial_position.rs).
    pub star_frame: [V3; 3],
}

/// A linear color's luminance weights (Rec. 709), constant.wgsl's LUMA.
pub(crate) const LUMA: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// A color's luminance, for which of two lights is the brighter.
fn luminance(c: [f64; 3]) -> f64 {
    dot3(&LUMA, &c)
}

/// The lane `light` lights a frame with.
pub fn write_environment(o: &mut [f32; ENV_FLOATS], light: &Light) {
    let c = &light.theme;
    // The terrain is lit by both the sun and the moon, summed: the
    // brighter is the frame's sun, which casts the shadow, and the other
    // is the second light, unshadowed. Summed rather than the brighter
    // alone, so the light does not snap from one direction to the other
    // where their intensities cross during a dusk or dawn ease.
    let sun_dir = *norm3(&light.sun, &mut [0.0; 3]);
    let sun_color = sun_light(light.altitude_km, sun_dir[2], white());
    let moon_color = MOON_COLOR.map(|v| v as f64 * light.moon_intensity);
    let sun = (sun_dir, sun_color);
    let moon_dir = *norm3(&light.moon, &mut [0.0; 3]);
    let moon = (moon_dir, moon_color);
    let (first, other) = if luminance(moon_color) > luminance(sun_color) { (moon, sun) } else { (sun, moon) };
    for (at, (d, color)) in [(0, first), (ENV_OTHER_LIGHT, other)] {
        put3(o, at, f32s(&d));
        put3(o, at + 4, [color[0] as f32, color[1] as f32, color[2] as f32]);
    }
    o[7] = light.fog_near as f32;
    let ambient_i = c[AMBIENT + 3];
    put3(o, 8, [c[AMBIENT] * ambient_i, c[AMBIENT + 1] * ambient_i, c[AMBIENT + 2] * ambient_i]);
    o[11] = light.fog_far as f32;
    o[15] = light.floor as f32;

    o[ENV_SKY] = light.stars as f32;
    o[ENV_SKY + 1] = light.time as f32;

    o[ENV_AIR] = light.altitude_km as f32;
    o[ENV_AIR + 1] = light.km_per_u as f32;
    o[ENV_AIR + 2] = light.latitude as f32;
    o[ENV_AIR + 3] = light.ground_km as f32;
    put3(o, ENV_SUN, f32s(&sun_dir));

    // The moonlit sky: the moon's phase, from the angle between it and the
    // sun as the observer sees them, and faded out as the sun comes up to
    // the horizon, past which the day's sky is the one the eye is adapted to
    // and a moon's is a four hundred thousandth of it.
    let elongation = dot3(&sun_dir, &moon_dir).clamp(-1.0, 1.0).acos();
    let night = 1.0 - smoothstep(-0.1, 0.0, sun_dir[2]);
    put3(o, ENV_MOON, f32s(&moon_dir));
    o[ENV_MOON + 3] = (MOONLIT_SKY * moon_phase_light(core::f64::consts::PI - elongation) * night) as f32;

    // The stars' frame, laid out a column at a time.
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
