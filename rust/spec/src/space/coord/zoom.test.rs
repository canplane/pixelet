// pxl-spec/space/coord/zoom.test.rs

use super::*;

#[test]
fn zoom_power_of_two() {
    for zoom in [27, UNIT_ZOOM, 18, 15] {
        assert_eq!(of_side(units(zoom)), zoom);
    }
    assert_eq!(cnt(UNIT_ZOOM, 15) as f64, units(15));
}
