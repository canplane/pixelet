// pxl-spec/space/camera/grid.test.rs

use super::*;

#[test]
fn field_of_view() {
    let deg = fov().to_degrees();
    assert!((deg - 53.130_102_354_155_99).abs() < 1e-12, "{deg}");
}

#[test]
fn zoom_cell_dist() {
    use crate::space::coord::zoom::{units, UNIT_ZOOM};
    for z in [0, UNIT_ZOOM, UNIT_ZOOM + 5] {
        assert_eq!(
            units(z) / split_dist(z),
            f64::from(SPANS) / f64::from(CELLS_A_RAD),
            "z{z}"
        );
        let d = split_dist(z);
        assert_eq!(
            cell_zoom_at((d * d) as f32),
            z,
            "z{z} begins at its split distance"
        );
    }
}
