// pxl-render/look/atmosphere.rs

//! The Earth's atmosphere, its numbers here only: the GPU's tables are drawn
//! from them, and every light through the air is read off those tables. Bruneton and Hillaire (EGSR 2020) with Unreal's
//! Earth; km, and coefficients per km.

/// The map's sphere, so ground and horizon are one; the literature's 6,360 km
/// is 0.3% off, which the scattering does not see.
use pxl_spec::space::coord::globe;

pub const BOTTOM_RADIUS_KM: f64 = globe::MERCATOR_RADIUS_KM;
pub const TOP_RADIUS_KM: f64 = BOTTOM_RADIUS_KM + 100.0;
pub const RAYLEIGH_SCATTERING: [f64; 3] = [5.802e-3, 13.558e-3, 33.1e-3];
pub const RAYLEIGH_SCALE_HEIGHT_KM: f64 = 8.0;
/// The day's weather: Koschmieder's visual range, 30-50 km on a clear day; the
/// aerosol takes what Rayleigh leaves of it at sea level. Unreal's gave 179 km.
pub const VISUAL_RANGE_KM: f64 = 40.0;
/// What the aerosol scatters of what it takes: Unreal's (3.996e-3 of 4.44e-3),
/// OPAC's continental haze at green (Hess, Koepke and Schult, 1998).
pub const MIE_ALBEDO: f64 = 0.9;
pub const MIE_EXTINCTION: f64 = 3.912 / VISUAL_RANGE_KM - RAYLEIGH_SCATTERING[1];
pub const MIE_SCATTERING: f64 = MIE_ALBEDO * MIE_EXTINCTION;
pub const MIE_SCALE_HEIGHT_KM: f64 = 1.2;
pub const MIE_G: f64 = 0.8;
pub const OZONE_ABSORPTION: [f64; 3] = [0.650e-3, 1.881e-3, 0.085e-3];
pub const OZONE_CENTER_KM: f64 = 25.0;
pub const OZONE_HALF_WIDTH_KM: f64 = 15.0;
/// The Earth's mean albedo, for the light the ground bounces into the air.
pub const GROUND_ALBEDO: f64 = 0.3;

pub const SUN_ANGULAR_RADIUS: f64 = 0.004675;

/// For a sun of illuminance 1: a face under a high sun shows about its albedo.
pub const EXPOSURE: f64 = 3.0;
/// The share of the land's fall from noon shown: a sun 3° up shows two thirds
/// of a stop under noon, so a dusk still reads.
pub const SHOWN_FALL: f64 = 0.15;
/// How dark a night reads: dim, never black, the cities lit by then.
pub const NIGHT_STOPS: f64 = 1.5;

/// #9fb4d6, linear (atmosphere.test.rs holds it to it): light as the night
/// eye reads it, bluer than it is.
pub const NIGHT_COLOR: [f64; 3] = [0.3467, 0.4564, 0.6724];
/// The full Moon against the Sun as the night eye reads it, not the 1/400,000 it is.
pub const MOON_LIGHT: f64 = 0.007;
/// Airglow and starlight: 0.002 lx to the full Moon's 0.25, taken as one stop
/// rather than seven, so a moonless night still reads.
pub const NIGHT_SKY: f64 = MOON_LIGHT / 2.0;
/// A city's core against the Sun, per steradian: VIIRS reads ~100 nW/cm²/sr,
/// the Black Marble's white, of the Sun's 550 W/m² in that band; raised as the Moon is.
pub const CITY_LIGHT: f64 = 1e-3 / 550.0 * MOON_LIGHT * 400_000.0;
/// The Black Marble's own lit city, #F9F0DA, linear.
pub const CITY_COLOR: [f64; 3] = [0.9473, 0.8714, 0.7011];

/// Allen's Astrophysical Quantities: 0.026·α + 4·10⁻⁹·α⁴ magnitudes (degrees).
/// A half moon gives a tenth of a full one's light, not half.
pub fn moon_phase_light(alpha: f64) -> f64 {
    let a = alpha.to_degrees().abs();
    libm::pow(10f64, -0.4 * (0.026 * a + 4e-9 * (a * a) * (a * a)))
}

#[cfg(test)]
#[path = "atmosphere.test.rs"]
mod test;
