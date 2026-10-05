// pxl-spec/space/zoom.test.rs

use super::*;

#[test]
fn a_zoom_is_one_power_of_two_however_it_is_read() {
    for z in [27, UNIT_Z, 18, 15] {
        assert_eq!(of_side(units(z)), z);
        assert_eq!(count(z, z - 2) * count(z - 2, z - 5), count(z, z - 5));
    }
    assert_eq!(count(UNIT_Z, 15) as f64, units(15));
    assert_eq!(units(27), 0.125);
    assert_eq!(units(0), (1u64 << UNIT_Z) as f64, "z0 is the world");
}

/// A point's cell at a zoom holds it, either side of the origin and below a unit.
#[test]
fn a_point_lies_in_the_cell_whose_corner_is_under_it() {
    for z in [27, UNIT_Z, 18] {
        for at in [[0i64, 0, 0], [5, -3, 2], [-1, -1, -1], [100, 7, -40]] {
            assert_eq!(index_of(z, &base_of(z, at)), at);
            let inside = pvf3::add(&base_of(z, at), &base_of(FINEST_Z, [1, 1, 1]));
            assert_eq!(index_of(z, &inside), at, "z{z} {at:?}");
        }
    }
}
