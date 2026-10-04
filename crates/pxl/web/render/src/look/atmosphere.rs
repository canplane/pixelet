// pxl-render/look/atmosphere.rs

//! The Earth's atmosphere, whose numbers live here only: the GPU draws its
//! tables from them and the CPU integrates the sun's light for the terrain.
//! Bruneton and Hillaire (EGSR 2020) with Unreal's Earth parameters. Distances
//! in km, coefficients per km.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The map's sphere, so ground and horizon are one sphere; the literature's
/// 6,360 km differs by 0.3%, which the scattering does not see.
pub const BOTTOM_RADIUS_KM: f64 = crate::look::globe::MERCATOR_RADIUS_KM;
pub const TOP_RADIUS_KM: f64 = BOTTOM_RADIUS_KM + 100.0;
pub const RAYLEIGH_SCATTERING: [f64; 3] = [5.802e-3, 13.558e-3, 33.1e-3];
pub const RAYLEIGH_SCALE_HEIGHT_KM: f64 = 8.0;
pub const MIE_SCATTERING: f64 = 3.996e-3;
pub const MIE_EXTINCTION: f64 = 4.44e-3;
pub const MIE_SCALE_HEIGHT_KM: f64 = 1.2;
pub const MIE_G: f64 = 0.8;
/// Koschmieder's visual range: 30-50 km on a clear day. Bruneton's medium alone
/// gives 179 km, so far ground stayed sharp against the sky.
pub const HAZE_VISUAL_RANGE_KM: f64 = 40.0;
/// Aerial perspective only: scaling the medium's own Mie this far would whiten the sky.
pub const HAZE_EXTINCTION: f64 = 3.912 / HAZE_VISUAL_RANGE_KM - (RAYLEIGH_SCATTERING[1] + MIE_EXTINCTION);
pub const HAZE_SCALE_HEIGHT_KM: f64 = 1.0;
pub const OZONE_ABSORPTION: [f64; 3] = [0.650e-3, 1.881e-3, 0.085e-3];
pub const OZONE_CENTER_KM: f64 = 25.0;
pub const OZONE_HALF_WIDTH_KM: f64 = 15.0;
/// The Earth's mean albedo, for the light the ground bounces into the air.
pub const GROUND_ALBEDO: f64 = 0.3;

/// The palette's forest (land.green_deep), what covers most land a wide view
/// takes in, so the terrain's edge runs into ground of its own color.
pub fn land() -> [f64; 3] {
    use pxl_spec::bake::format::GroundChannelKey;
    use pxl_spec::cell::cube_types;
    let swatch = pxl_spec::cell::color::ground(GroundChannelKey::LAND, cube_types::land::green_deep).or_bug(Errno::ENOTRECOVERABLE);
    crate::look::color::linear_rgb(swatch.rgb)
}
pub const SUN_ANGULAR_RADIUS: f64 = 0.004675;

/// The one eye the sky and the ground are seen by: the lights are for a sun of
/// illuminance 1, and this shows a face under a high sun at about its albedo.
pub const EXPOSURE: f64 = 3.0;
/// A half, not one: the illuminance falls with the direct sun long before the
/// sky dims, and following it fully washes a low sun's sky out white.
pub const ADAPTATION: f64 = 0.5;
/// Seven stops: how far the eye opens once the sun is gone, which is how bright
/// a night reads. The moon and the night sky do not close it.
pub const MAX_ADAPTATION: f64 = 128.0;

/// #9fb4d6, linear: light as the night eye reads it, bluer than it is.
pub const NIGHT_COLOR: [f64; 3] = [0.3467, 0.4564, 0.6724];
/// The full Moon against the Sun, as the night eye reads it rather than the
/// 1/400,000 it is.
pub const MOON_LIGHT: f64 = 0.007;
/// A clear moonless sky's own glow, airglow and starlight: 0.002 lx against
/// the full Moon's 0.25, taken as one stop rather than seven, so a moonless
/// night is read by it and the Moon only brightens it.
pub const NIGHT_SKY: f64 = MOON_LIGHT / 2.0;

/// Allen's Astrophysical Quantities: 0.026·α + 4·10⁻⁹·α⁴ magnitudes (degrees).
/// A half moon gives a tenth of a full one's light, not half.
pub fn moon_phase_light(alpha: f64) -> f64 {
    let a = alpha.to_degrees().abs();
    libm::pow(10f64, -0.4 * (0.026 * a + 4e-9 * a.powi(4)))
}

const STEPS: u32 = 64;

fn extinction(h: f64) -> [f64; 3] {
    let r = libm::exp(-h / RAYLEIGH_SCALE_HEIGHT_KM);
    let m = libm::exp(-h / MIE_SCALE_HEIGHT_KM);
    let o = (1.0 - (h - OZONE_CENTER_KM).abs() / OZONE_HALF_WIDTH_KM).max(0.0);
    core::array::from_fn(|k| RAYLEIGH_SCATTERING[k] * r + MIE_EXTINCTION * m + OZONE_ABSORPTION[k] * o)
}

fn horizon_mu(r: f64) -> f64 {
    -(1.0 - (BOTTOM_RADIUS_KM / r).powi(2)).max(0.0).sqrt()
}

/// Zero for a direction the ground stands in.
pub fn transmittance(altitude_km: f64, mu: f64) -> [f64; 3] {
    let r = BOTTOM_RADIUS_KM + altitude_km.max(0.0);
    if mu < horizon_mu(r) {
        return [0.0; 3];
    }
    let d = -r * mu + (r * r * (mu * mu - 1.0) + TOP_RADIUS_KM * TOP_RADIUS_KM).max(0.0).sqrt();
    let dt = d / STEPS as f64;
    let mut depth = [0.0; 3];
    for i in 0..STEPS {
        let t = (i as f64 + 0.5) * dt;
        let h = (t * t + 2.0 * r * mu * t + r * r).sqrt() - BOTTOM_RADIUS_KM;
        let e = extinction(h);
        for k in 0..3 {
            depth[k] += e[k] * dt;
        }
    }
    depth.map(|d| libm::exp(-d))
}

/// The inverse of what the air leaves of a sun overhead at sea level.
pub fn white() -> [f64; 3] {
    static WHITE: std::sync::LazyLock<[f64; 3]> = std::sync::LazyLock::new(|| transmittance(0.0, 1.0).map(|t| 1.0 / t));
    *WHITE
}

/// The disc sets across its width rather than switching off at its center
/// (atmos_lut.wgsl's sun_transmittance).
pub fn light_at(altitude_km: f64, mu: f64) -> [f64; 3] {
    let r = BOTTOM_RADIUS_KM + altitude_km.max(0.0);
    let horizon = horizon_mu(r);
    let above = ((mu - horizon) / (2.0 * SUN_ANGULAR_RADIUS) + 0.5).clamp(0.0, 1.0);
    if above == 0.0 {
        return [0.0; 3];
    }
    transmittance(altitude_km, mu.max(horizon + 1e-4)).map(|t| t * above)
}
