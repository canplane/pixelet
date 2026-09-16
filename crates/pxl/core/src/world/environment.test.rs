// pxl_core/world/environment.test.rs

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

/// Seoul's day of 2024-06-20 (local midnight 2024-06-19T15:00:00Z): sunrise
/// at 05:16, sunset at 19:52. The tolerance is 0.002 hours, about seven
/// seconds — the five-minute sampling interpolates, so what is pinned is the
/// crossing, not the sample it fell between.
#[test]
fn seoul_at_the_solstice_has_its_longest_day() {
    let mut env = Environment::default();
    env.daylight(37.5665, 126.978, 1_718_809_200_000.0);
    assert!((env.daylight[0] - 5.262678956569208).abs() < 0.002, "sunrise {}", env.daylight[0]);
    assert!((env.daylight[1] - 19.861559470758326).abs() < 0.002, "sunset {}", env.daylight[1]);
}
