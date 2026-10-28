// pxl-kernel/domain/control/interaction/collision.test.rs

use super::*;

#[test]
fn a_cube_goes_on_anything() {
    for under in [Kind::cube, Kind::decor, Kind::marker] {
        assert!(goes_on(Kind::cube, under), "on a {under:?}");
    }
}

#[test]
fn a_decor_or_a_marker_goes_on_a_cube_or_a_decor_and_on_no_marker() {
    for tool in [Kind::decor, Kind::marker] {
        assert!(goes_on(tool, Kind::cube), "a {tool:?} on a cube");
        assert!(goes_on(tool, Kind::decor), "a {tool:?} on a decor");
        assert!(!goes_on(tool, Kind::marker), "a {tool:?} on a marker");
    }
}
