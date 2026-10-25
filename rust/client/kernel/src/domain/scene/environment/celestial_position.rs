// pxl-kernel/domain/scene/environment/celestial_position.rs

//! Where the sun, the moon and the stars are, by the standard low-precision
//! formulas: the sun to about 0.01°, the moon to roughly 0.3–1°.

const DEG: f64 = core::f64::consts::PI / 180.0;

fn norm360(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
}

/// The sun's right ascension and declination, and the obliquity, in degrees.
fn sun_equatorial(d: f64) -> (f64, f64, f64) {
    let mean_long = norm360(280.46 + 0.985_647_4 * d);
    let g = norm360(357.528 + 0.985_600_3 * d) * DEG;
    let lon = norm360(mean_long + 1.915 * libm::sin(g) + 0.02 * libm::sin(2.0 * g)) * DEG;
    let obliquity = 23.439 - 0.000_000_4 * d;
    let e = obliquity * DEG;
    let ra = norm360(libm::atan2(libm::cos(e) * libm::sin(lon), libm::cos(lon)) / DEG);
    let dec = libm::asin(libm::sin(e) * libm::sin(lon)) / DEG;
    (ra, dec, obliquity)
}

/// The moon's right ascension and declination, in degrees.
fn moon_equatorial(d: f64, obliquity: f64) -> (f64, f64) {
    let mean_long = norm360(218.316 + 13.176_396 * d);
    let mean_anomaly = norm360(134.963 + 13.064_993 * d);
    let arg_latitude = norm360(93.272 + 13.229_35 * d);
    let lon = norm360(mean_long + 6.289 * libm::sin(mean_anomaly * DEG)) * DEG;
    let lat = 5.128 * libm::sin(arg_latitude * DEG) * DEG;
    let e = obliquity * DEG;
    let ra = norm360(
        libm::atan2(
            libm::sin(lon) * libm::cos(e) - libm::tan(lat) * libm::sin(e),
            libm::cos(lon),
        ) / DEG,
    );
    let dec =
        libm::asin(libm::sin(lat) * libm::cos(e) + libm::cos(lat) * libm::sin(e) * libm::sin(lon))
            / DEG;
    (ra, dec)
}

/// Degrees, `d` days after J2000.0.
fn local_sidereal(d: f64, lon: f64) -> f64 {
    let gmst = norm360(280.460_618_37 + 360.985_647_366_29 * d);
    norm360(gmst + lon)
}

/// X east, Y north, Z up, at distance 100: right ascension `ra` and
/// declination `dec`, in degrees, turned by the sky's one rotation
/// (`equatorial_to_local`).
fn local_direction(ra: f64, dec: f64, lat: f64, lon: f64, d: f64) -> [f64; 3] {
    let m = equatorial_to_local(d, lat, lon);
    let ((sa, ca), (sd, cd)) = (libm::sincos(ra * DEG), libm::sincos(dec * DEG));
    let e = [cd * ca, cd * sa, sd];
    core::array::from_fn(|r| 100.0 * (m[r][0] * e[0] + m[r][1] * e[1] + m[r][2] * e[2]))
}

/// Days since J2000.0 at Unix milliseconds `unix_ms`.
pub fn j2000_days(unix_ms: f64) -> f64 {
    unix_ms / 86_400_000.0 + 2_440_587.5 - 2_451_545.0
}

pub fn sun_position(d: f64, lat: f64, lon: f64) -> [f64; 3] {
    let (ra, dec, _) = sun_equatorial(d);
    local_direction(ra, dec, lat, lon, d)
}

/// Equatorial (X to the vernal equinox, Z to the pole) to local (X east,
/// Y north, Z up), as rows: the sky's one rotation, the sun's, the moon's
/// and the nine thousand stars'.
pub fn equatorial_to_local(d: f64, lat: f64, lon: f64) -> [[f64; 3]; 3] {
    let (sl, cl) = libm::sincos(local_sidereal(d, lon) * DEG);
    let (sp, cp) = libm::sincos(lat * DEG);
    [
        [-sl, cl, 0.0],
        [-sp * cl, -sp * sl, cp],
        [cp * cl, cp * sl, sp],
    ]
}

pub fn moon_position(d: f64, lat: f64, lon: f64) -> [f64; 3] {
    let (_, _, obliquity) = sun_equatorial(d);
    let (ra, dec) = moon_equatorial(d, obliquity);
    local_direction(ra, dec, lat, lon, d)
}
