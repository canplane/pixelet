// pxl-kernel/layer/environment/state.test.rs

use super::*;

/// Seoul, 2024-06-20: sunrise 05:16, sunset 19:52, to 0.002 h (~7 s), since
/// the sampling interpolates the crossing.
#[test]
fn seoul_at_the_solstice_has_its_longest_day() {
    let mut env = EnvironmentDraw::default();
    env.daylight(37.5665, 126.978, 1_718_809_200_000.0);
    assert!((env.daylight[0] - 5.262678956569208).abs() < 0.002, "sunrise {}", env.daylight[0]);
    assert!((env.daylight[1] - 19.861559470758326).abs() < 0.002, "sunset {}", env.daylight[1]);
}
