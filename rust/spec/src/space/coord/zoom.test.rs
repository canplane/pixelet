// pxl-spec/space/coord/zoom.test.rs

use super::*;

#[test]
fn zoom_power_of_two() {
    for zoom in [27, UNIT_ZOOM, 18, 15] {
        assert_eq!(of_side(units(zoom)), zoom);
    }
    assert_eq!(count(UNIT_ZOOM, 15) as f64, units(15));
}

/// A point's cell at a zoom holds it, either side of the origin and below a unit.
#[test]
fn point_cell_corner() {
    for zoom in [27, UNIT_ZOOM, 18] {
        for at in [[0i32, 0, 0], [5, -3, 2], [-1, -1, -1], [100, 7, -40]] {
            assert_eq!(index_of(zoom, &base_of(zoom, at)), at);
            let inside = base_of(zoom, at).map(|v| v + 1);
            assert_eq!(index_of(zoom, &inside), at, "z{zoom} {at:?}");
        }
    }
}
