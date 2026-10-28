// pxl-spec/space/camera/grid.test.rs

use super::*;

#[test]
fn the_view_s_height_spans_a_unit_at_unit_distance() {
    let half = libm::tan(FOV_DEG.to_radians() / 2.0);
    assert!((half - 0.5).abs() < 1e-15, "{half}");
}
