// pxl-kernel/layer/environment/celestial_position.test.rs

use super::*;

/// Seoul at local noon the day before the solstice: altitude near 74°, a
/// little east of south.
#[test]
fn the_sun_at_seoul_noon_is_high_and_south_south_east() {
    let d = j2000_days(1_718_852_400_000.0);
    let s = sun_position(d, 37.5665, 126.978);
    let want = [13.449099302276881, -23.813088623065383, 96.18762154345812];
    for k in 0..3 {
        assert!((s[k] - want[k]).abs() < 1e-9, "axis {k}: {} vs {}", s[k], want[k]);
    }
}

#[test]
fn the_stars_turn_with_the_sun_and_the_moon() {
    let d = j2000_days(1_718_852_400_000.0);
    for (lat, lon) in [(37.5665, 126.978), (-33.87, 151.21)] {
        let m = equatorial_to_local(d, lat, lon);
        for (ra, dec) in [(101.287, -16.716), (37.955, 89.264), (200.0, 0.0)] {
            let (a, b) = (ra * DEG, dec * DEG);
            let e = [libm::cos(b) * libm::cos(a), libm::cos(b) * libm::sin(a), libm::sin(b)];
            let got: [f64; 3] = core::array::from_fn(|r| m[r][0] * e[0] + m[r][1] * e[1] + m[r][2] * e[2]);
            let want = local_direction(ra, dec, lat, lon, d);
            for k in 0..3 {
                assert!((got[k] * 100.0 - want[k]).abs() < 1e-6, "({ra}, {dec}) at {lat}: axis {k}: {} vs {}", got[k] * 100.0, want[k]);
            }
        }
    }
}
