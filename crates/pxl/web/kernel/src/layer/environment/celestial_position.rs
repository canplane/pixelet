// pxl-kernel/layer/environment/celestial_position.rs

//! Where the sun, the moon and the stars are, by the standard low-precision
//! formulas: the sun to about 0.01°, the moon to roughly 0.3–1°.

use pxl_spec::space::v3::V3;

const DEG: f64 = core::f64::consts::PI / 180.0;

fn norm360(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// The sun's right ascension and declination, and the obliquity, in degrees.
fn sun_equatorial(d: f64) -> (f64, f64, f64) {
    let mean_long = norm360(280.46 + 0.985_647_4 * d);
    let g = norm360(357.528 + 0.985_600_3 * d) * DEG;
    let lon = norm360(mean_long + 1.915 * g.sin() + 0.02 * (2.0 * g).sin()) * DEG;
    let obliquity = 23.439 - 0.000_000_4 * d;
    let e = obliquity * DEG;
    let ra = norm360((e.cos() * lon.sin()).atan2(lon.cos()) / DEG);
    let dec = (e.sin() * lon.sin()).asin() / DEG;
    (ra, dec, obliquity)
}

/// The moon's right ascension and declination, in degrees.
fn moon_equatorial(d: f64, obliquity: f64) -> (f64, f64) {
    let mean_long = norm360(218.316 + 13.176_396 * d);
    let mean_anomaly = norm360(134.963 + 13.064_993 * d);
    let arg_latitude = norm360(93.272 + 13.229_35 * d);
    let lon = norm360(mean_long + 6.289 * (mean_anomaly * DEG).sin()) * DEG;
    let lat = 5.128 * (arg_latitude * DEG).sin() * DEG;
    let e = obliquity * DEG;
    let ra = norm360((lon.sin() * e.cos() - lat.tan() * e.sin()).atan2(lon.cos()) / DEG);
    let dec = (lat.sin() * e.cos() + lat.cos() * e.sin() * lon.sin()).asin() / DEG;
    (ra, dec)
}

/// Degrees, `d` days after J2000.0.
fn local_sidereal(d: f64, lon: f64) -> f64 {
    let gmst = norm360(280.460_618_37 + 360.985_647_366_29 * d);
    norm360(gmst + lon)
}

/// X east, Y north, Z up, at distance 100. Azimuth by Duffett-Smith's
/// construction.
fn local_direction(ra: f64, dec: f64, lat: f64, lon: f64, d: f64) -> V3 {
    let lst = local_sidereal(d, lon);
    let hour_angle = (lst - ra + 180.0).rem_euclid(360.0) - 180.0;
    let (ha, dec, lat) = (hour_angle * DEG, dec * DEG, lat * DEG);
    let alt = (dec.sin() * lat.sin() + dec.cos() * lat.cos() * ha.cos()).asin();
    let cos_az = ((dec.sin() - alt.sin() * lat.sin()) / (alt.cos() * lat.cos())).clamp(-1.0, 1.0);
    let mut az = cos_az.acos();
    if ha.sin() > 0.0 {
        az = 2.0 * core::f64::consts::PI - az;
    }
    let horizontal = alt.cos() * 100.0;
    [az.sin() * horizontal, az.cos() * horizontal, alt.sin() * 100.0]
}

/// Days since J2000.0 at Unix milliseconds `unix_ms`.
pub fn j2000_days(unix_ms: f64) -> f64 {
    unix_ms / 86_400_000.0 + 2_440_587.5 - 2_451_545.0
}

pub fn sun_position(d: f64, lat: f64, lon: f64) -> V3 {
    let (ra, dec, _) = sun_equatorial(d);
    local_direction(ra, dec, lat, lon, d)
}

/// Equatorial (X to the vernal equinox, Z to the pole) to local (X east,
/// Y north, Z up), as rows: the same sky as `local_direction` as one matrix
/// for the nine thousand stars.
pub fn equatorial_to_local(d: f64, lat: f64, lon: f64) -> [V3; 3] {
    let (sl, cl) = (local_sidereal(d, lon) * DEG).sin_cos();
    let (sp, cp) = (lat * DEG).sin_cos();
    [[-sl, cl, 0.0], [-sp * cl, -sp * sl, cp], [cp * cl, cp * sl, sp]]
}

pub fn moon_position(d: f64, lat: f64, lon: f64) -> V3 {
    let (_, _, obliquity) = sun_equatorial(d);
    let (ra, dec) = moon_equatorial(d, obliquity);
    local_direction(ra, dec, lat, lon, d)
}

#[cfg(test)]
#[path = "celestial_position.test.rs"]
mod test;
