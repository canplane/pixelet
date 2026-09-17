// pxl_core/module/map/environment.test.rs

use super::*;

/// Seoul's day of 2024-06-20 (local midnight 2024-06-19T15:00:00Z): sunrise
/// at 05:16, sunset at 19:52. The tolerance is 0.002 hours, about seven
/// seconds — the five-minute sampling interpolates, so what is pinned is the
/// crossing, not the sample it fell between.
#[test]
fn seoul_at_the_solstice_has_its_longest_day() {
    let mut env = EnvironmentDraw::default();
    env.daylight(37.5665, 126.978, 1_718_809_200_000.0);
    assert!((env.daylight[0] - 5.262678956569208).abs() < 0.002, "sunrise {}", env.daylight[0]);
    assert!((env.daylight[1] - 19.861559470758326).abs() < 0.002, "sunset {}", env.daylight[1]);
}
