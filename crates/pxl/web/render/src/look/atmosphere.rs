// pxl-render/look/atmosphere.rs

//! The Earth's atmosphere as the frame is lit through it: one model, whose
//! numbers live here and nowhere else. The GPU reads them from the
//! atmosphere uniform (gpu/atmosphere.rs lays it out) to draw its tables and
//! the sky from them (frame/shader/atmos_*.wgsl); the CPU integrates the
//! same model along the sun's ray to color the light the terrain is lit by,
//! which is one number a frame and not worth a texture read a fragment.
//!
//! The model is Bruneton's and Hillaire's ("A Scalable and Production Ready
//! Sky and Atmosphere Rendering Technique", EGSR 2020), with the parameters
//! their papers and Unreal's SkyAtmosphere take as the Earth: Rayleigh
//! scattering falling off over 8 km, Mie over 1.2 km, and an ozone layer
//! that absorbs and does not scatter, a tent peaking at 25 km. Distances are
//! kilometers and coefficients per kilometer throughout.

use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// The planet is the map's (look/globe.rs), so the ground the world is laid
/// on and the horizon the sky is drawn with are one sphere; the literature's
/// 6,360 km differs by a third of a percent, which the scattering does not see.
pub const BOTTOM_RADIUS_KM: f64 = crate::look::globe::MERCATOR_RADIUS_KM;
pub const TOP_RADIUS_KM: f64 = BOTTOM_RADIUS_KM + 100.0;
pub const RAYLEIGH_SCATTERING: [f64; 3] = [5.802e-3, 13.558e-3, 33.1e-3];
pub const RAYLEIGH_SCALE_HEIGHT_KM: f64 = 8.0;
pub const MIE_SCATTERING: f64 = 3.996e-3;
pub const MIE_EXTINCTION: f64 = 4.44e-3;
pub const MIE_SCALE_HEIGHT_KM: f64 = 1.2;
/// The Mie phase's asymmetry: how strongly haze throws light forward, which
/// is the glow around the sun.
pub const MIE_G: f64 = 0.8;
/// How far the eye sees through the air near the ground, in kilometers:
/// Koschmieder's visual range, `3.912/β`, which is 30 to 50 km on a clear
/// day on Earth and under 10 in haze.
///
/// Bruneton's medium alone gives 179 km — cleaner than the clearest day
/// anyone stands in — because it carries no aerosol in the boundary layer,
/// only the stratospheric Mie the sky's color wants. Without one the ground
/// stays sharp to the horizon and something else has to hide where it ends
/// (issue/20260923-the-ground-runs-to-the-horizon.md).
pub const HAZE_VISUAL_RANGE_KM: f64 = 40.0;
/// The aerosol the range asks for, over what the medium already extinguishes
/// at green, where the eye's contrast is measured. Read by the aerial
/// perspective alone: scaling the medium's own Mie this far would whiten the
/// sky, which is why the haze is a layer of its own.
pub const HAZE_EXTINCTION: f64 = 3.912 / HAZE_VISUAL_RANGE_KM - (RAYLEIGH_SCATTERING[1] + MIE_EXTINCTION);
/// The layer it sits in: the boundary layer the aerosol is mixed through,
/// so a view from above passes through less of it, as one on Earth does.
pub const HAZE_SCALE_HEIGHT_KM: f64 = 1.0;
pub const OZONE_ABSORPTION: [f64; 3] = [0.650e-3, 1.881e-3, 0.085e-3];
pub const OZONE_CENTER_KM: f64 = 25.0;
pub const OZONE_HALF_WIDTH_KM: f64 = 15.0;
/// What the planet's surface reflects, for the light the ground bounces back
/// into the air: the Earth's mean, a gray the scattering sees and the eye
/// does not. The ground the eye sees past the terrain is `land` below.
pub const GROUND_ALBEDO: f64 = 0.3;

/// The ground seen where the world has no terrain drawn, past its edge and
/// from high up: the palette's forest (spec/pxl/spec/cell/color.json's
/// land.green_deep), which is what covers most of the land a view that wide
/// takes in, in linear light — so the terrain's edge runs into ground of its
/// own color rather than a gray.
pub fn land() -> [f64; 3] {
    use pxl_spec::bake::format::GroundChannelKey;
    use pxl_spec::cell::cube_types;
    let swatch = pxl_spec::cell::color::ground(GroundChannelKey::LAND, cube_types::land::green_deep).or_bug(Errno::ENOTRECOVERABLE);
    crate::look::color::linear_rgb(swatch.rgb)
}
/// The sun's disc, as seen from the Earth.
pub const SUN_ANGULAR_RADIUS: f64 = 0.004675;

/// The radiance the tables hold is for a sun of illuminance 1; this is what
/// a frame shows it at under a sun overhead, which is the scale the terrain
/// is lit on — and what the eye adapts from as the light falls (MAX_ADAPTATION). Under the
/// day theme a horizontal face takes its albedo times about 1.24 — the sun
/// at SUN_LIGHT and the fill, ambient 0.68 and hemisphere 0.82 over pi, and
/// the floor's 0.6 — where the air lights the same face with its albedo
/// times about 0.28 — the sun through the air at a midday height over pi,
/// and the sky's irradiance. Their ratio puts the air's haze and the ground
/// seen through it on the terrain's scale; past it the ground drowns white.
pub const EXPOSURE: f64 = 4.0;
/// How the exposure rises as the light falls: the eye adapts to the
/// horizontal illuminance, the sky's and the sun's, taking a sun overhead at
/// sea level as 1 — which the white balance makes exactly 1 — by this power
/// of its fall. A half and not the whole: the illuminance falls with the
/// direct sun long before the sky dims, and following it fully washes a low
/// sun's sky out white; an eye and a camera's metering both leave a dim
/// scene dimmer than a bright one.
pub const ADAPTATION: f64 = 0.5;
/// How far it rises at most, as a factor over EXPOSURE: six stops, so a
/// sunset's sky shows as the color it is rather than dark; past it the
/// twilight falls away to night, as it does to an eye.
pub const MAX_ADAPTATION: f64 = 64.0;
/// The terrain's direct sunlight at noon, before the air colors it: the
/// directional intensity the daylight themes were tuned at, so the ground is
/// no brighter or darker than it was under them.
pub const SUN_LIGHT: f64 = 0.5;

/// A full moon's sky, as a share of a clear day's, the way the frame shows it
/// with the sun down. The moon's light is some four hundred thousandth of
/// the sun's and an eye adapts to it by rods where no screen can, so the
/// moonlit sky is drawn from the same tables at the brightness it is seen at
/// rather than the one it has: the day's sky, dimmed as a moonlit night
/// reads next to a day — a full moon's zenith a deep navy on an eight-bit
/// screen, and a half moon's, a tenth of it, nearly black.
pub const MOONLIT_SKY: f64 = 0.15;

/// The moon's light at a phase angle `alpha` (radians; 0 full, pi new),
/// against a full moon's: its magnitude's rise with the phase angle
/// (Allen's Astrophysical Quantities, 0.026·α + 4·10⁻⁹·α⁴ in degrees), which
/// falls far faster than the lit fraction — a half moon gives a tenth of the
/// full one's light, not half, as the lit face's shadows turn toward the eye.
pub fn moon_phase_light(alpha: f64) -> f64 {
    let a = alpha.to_degrees().abs();
    10f64.powf(-0.4 * (0.026 * a + 4e-9 * a.powi(4)))
}

/// Steps the CPU's transmittance takes to the top of the atmosphere.
const STEPS: u32 = 64;

/// The extinction per kilometer at altitude `h` km, by channel.
fn extinction(h: f64) -> [f64; 3] {
    let r = (-h / RAYLEIGH_SCALE_HEIGHT_KM).exp();
    let m = (-h / MIE_SCALE_HEIGHT_KM).exp();
    let o = (1.0 - (h - OZONE_CENTER_KM).abs() / OZONE_HALF_WIDTH_KM).max(0.0);
    core::array::from_fn(|k| RAYLEIGH_SCATTERING[k] * r + MIE_EXTINCTION * m + OZONE_ABSORPTION[k] * o)
}

/// The cosine below which a ray from radius `r` meets the ground.
fn horizon_mu(r: f64) -> f64 {
    -(1.0 - (BOTTOM_RADIUS_KM / r).powi(2)).max(0.0).sqrt()
}

/// What reaches a point at `altitude_km` of light arriving from a direction
/// whose cosine from the local zenith is `mu`, having crossed the atmosphere
/// from its top, by channel. Zero for a direction the ground stands in.
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
    depth.map(|d| (-d).exp())
}

/// The white the frame is balanced to: the inverse of what the air leaves of
/// a sun overhead at sea level. Light that has crossed the least air there
/// is reads as white, as an eye adapted to daylight takes it; a lower
/// sun is warmer than that and a sky bluer. The model's, so taken once.
pub fn white() -> [f64; 3] {
    static WHITE: std::sync::LazyLock<[f64; 3]> = std::sync::LazyLock::new(|| transmittance(0.0, 1.0).map(|t| 1.0 / t));
    *WHITE
}

/// What reaches a point at `altitude_km` of a light at `mu` from the zenith,
/// by channel, the ground's shadow included (atmos_lut.wgsl's
/// sun_transmittance, on the CPU). The disc sets over the horizon rather
/// than switching off at its center: the part still above the horizon, taken
/// linearly across the disc's width.
pub fn light_at(altitude_km: f64, mu: f64) -> [f64; 3] {
    let r = BOTTOM_RADIUS_KM + altitude_km.max(0.0);
    let horizon = horizon_mu(r);
    let above = ((mu - horizon) / (2.0 * SUN_ANGULAR_RADIUS) + 0.5).clamp(0.0, 1.0);
    if above == 0.0 {
        return [0.0; 3];
    }
    transmittance(altitude_km, mu.max(horizon + 1e-4)).map(|t| t * above)
}

/// The sunlight the terrain at `altitude_km` is lit by, with the sun at
/// `sun_mu` from the zenith, balanced to `white`.
pub fn sun_light(altitude_km: f64, sun_mu: f64, white: [f64; 3]) -> [f64; 3] {
    let t = light_at(altitude_km, sun_mu);
    core::array::from_fn(|k| SUN_LIGHT * t[k] * white[k])
}
