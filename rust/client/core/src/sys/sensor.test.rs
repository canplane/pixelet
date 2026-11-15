// pxl-core/sys/sensor.test.rs

use super::*;

/// A position is kept where the device put it. Only its altitude is read at
/// the leaf tile's corner: over sea level by the geoid there, which lies
/// within a hair of the geoid at the position.
#[test]
fn a_position_is_kept_where_it_is() {
    let (lon, lat) = (126.9784, 37.5667);
    set_pos(Some(Position {
        lon,
        lat,
        alt: Some(100.0),
    }));
    let (got, alt) = pos().expect("a position");
    assert_eq!(got, [lon, lat]);
    let alt = alt.expect("an altitude");
    let want = 100.0 - epsg4326::geoid_m(lon, lat);
    assert!((alt - want).abs() < 0.05, "{alt} against {want}");
}
