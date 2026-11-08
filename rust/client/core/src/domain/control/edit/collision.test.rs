// pxl-core/domain/control/edit/collision.test.rs

use super::*;

#[test]
fn cube_on_anything() {
    for under in [Kind::cube, Kind::decor, Kind::marker] {
        assert!(stands_on(Kind::cube, under), "on a {under:?}");
    }
}

#[test]
fn decor_marker_support() {
    for tool in [Kind::decor, Kind::marker] {
        assert!(stands_on(tool, Kind::cube), "a {tool:?} on a cube");
        assert!(stands_on(tool, Kind::decor), "a {tool:?} on a decor");
        assert!(!stands_on(tool, Kind::marker), "a {tool:?} on a marker");
    }
}
