// pxl_core/layer/environment/celestial_position.test.rs

use super::*;

/// Seoul at local noon on the day before the solstice: the direction comes
/// back with Z at 96.2 of 100, an altitude near 74°, and X and Y putting it
/// a little east of south.
#[test]
fn the_sun_at_seoul_noon_is_high_and_south_south_east() {
    let d = 1_718_852_400_000.0 / 86_400_000.0 + 2_440_587.5 - 2_451_545.0;
    let s = sun_position(d, 37.5665, 126.978);
    let want = [13.449099302276881, -23.813088623065383, 96.18762154345812];
    for k in 0..3 {
        assert!((s[k] - want[k]).abs() < 1e-9, "axis {k}: {} vs {}", s[k], want[k]);
    }
}
