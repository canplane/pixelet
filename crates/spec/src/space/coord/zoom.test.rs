// pxl-spec/space/coord/zoom.test.rs

use super::*;

/// A 512u square, as a size and nothing more.
const Z15: Zoom = 15;

#[test]
fn a_zoom_is_one_power_of_two_however_it_is_read() {
    for zoom in [27, UNIT_ZOOM, 18, 15] {
        assert_eq!(of_side(units(zoom)), zoom);
        assert_eq!(count(zoom, zoom - 2) * count(zoom - 2, zoom - 5), count(zoom, zoom - 5));
    }
    assert_eq!(count(UNIT_ZOOM, 15) as f64, units(15));
    assert_eq!(units(27), 0.125);
    assert_eq!(units(0), (1u64 << UNIT_ZOOM) as f64, "z0 is the world");
}

/// A point's cell at a zoom holds it, either side of the origin and below a unit.
#[test]
fn a_point_lies_in_the_cell_whose_corner_is_under_it() {
    for zoom in [27, UNIT_ZOOM, 18] {
        for at in [[0i64, 0, 0], [5, -3, 2], [-1, -1, -1], [100, 7, -40]] {
            assert_eq!(index_of(zoom, &base_of(zoom, at)), at);
            let inside = pvf3::add(&base_of(zoom, at), &base_of(FINEST_ZOOM, [1, 1, 1]));
            assert_eq!(index_of(zoom, &inside), at, "z{zoom} {at:?}");
        }
    }
}

#[test]
fn a_leaf_tile_s_index_and_base_round_trip_on_all_three_axes() {
    let addr = ivec::gather(-1, 2049, 1801);
    let at = idx(Z15, addr);
    assert_eq!(ivec::repr(at), "-1:4:3");
    assert_eq!(ivec::repr(from_idx(Z15, at)), "-512:2048:1536");
}
