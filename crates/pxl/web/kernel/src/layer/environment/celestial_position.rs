// pxl-kernel/layer/environment/celestial_position.rs

//! Where the sun, the moon and the stars are.
//!
//! The standard low-precision formulas: the sun good to about 0.01°, the
//! moon's single dominant term per angle to roughly 0.3–1°. Degrees until
//! the local direction, as they are conventionally published.

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

/// The local sidereal time at longitude `lon`, `d` days after J2000.0, in
/// degrees: the right ascension on the meridian.
fn local_sidereal(d: f64, lon: f64) -> f64 {
    let gmst = norm360(280.460_618_37 + 360.985_647_366_29 * d);
    norm360(gmst + lon)
}

/// Equatorial to the observer's local direction: X east, Y north, Z up, at
/// distance 100 as the three lights stood. Azimuth is the Duffett-Smith
/// construction, acos and a sign from the hour angle.
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

pub fn sun_position(d: f64, lat: f64, lon: f64) -> V3 {
    let (ra, dec, _) = sun_equatorial(d);
    local_direction(ra, dec, lat, lon, d)
}

/// The rotation from the equatorial frame — X to the vernal equinox, Z to
/// the celestial pole — to the observer's local one, X east, Y north, Z up,
/// as rows: what turns a star's cataloged direction into where it stands.
/// The same sky as `local_direction` reads one body at a time, as a matrix,
/// since the stars are nine thousand bodies and one sky: the hour angle is
/// the sidereal time less the right ascension, turned about the pole, and the
/// pole is tipped to the latitude.
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
