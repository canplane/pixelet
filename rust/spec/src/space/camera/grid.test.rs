// pxl-spec/space/camera/grid.test.rs

use super::*;

#[test]
fn the_view_s_field_is_53_13_degrees() {
    let deg = fov().to_degrees();
    assert!((deg - 53.130_102_354_155_99).abs() < 1e-12, "{deg}");
}

#[test]
fn a_zoom_s_cells_are_drawn_from_where_a_side_spans_spans_of_the_view_s() {
    use crate::space::coord::zoom::{units, UNIT_ZOOM};
    for z in [0, UNIT_ZOOM, UNIT_ZOOM + 5] {
        assert_eq!(
            units(z) / split_distance(z),
            pxl_spec_core::grid::SPANS as f64 * CELL,
            "z{z}"
        );
        let d = split_distance(z);
        assert_eq!(cell_at(d * d), z, "z{z} begins at its split distance");
    }
}
