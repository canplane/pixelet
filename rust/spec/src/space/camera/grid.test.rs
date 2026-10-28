// pxl-spec/space/camera/grid.test.rs

use super::*;

#[test]
fn the_view_s_height_spans_a_unit_at_unit_distance() {
    let half = libm::tan(FOV_DEG.to_radians() / 2.0);
    assert!((half - 0.5).abs() < 1e-15, "{half}");
}

#[test]
fn a_zoom_s_cells_are_drawn_from_where_a_side_spans_two_of_the_view_s() {
    use crate::space::coord::zoom::{units, UNIT_ZOOM};
    for z in [0, UNIT_ZOOM, UNIT_ZOOM + 5] {
        assert_eq!(units(z) / split_distance(z), 2.0 * CELL, "z{z}");
        let d = split_distance(z);
        assert_eq!(cell_at(d * d), z, "z{z} begins at its split distance");
    }
}
