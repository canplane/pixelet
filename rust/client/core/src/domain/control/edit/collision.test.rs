// pxl-core/domain/control/edit/collision.test.rs

use super::*;

#[test]
fn cube_on_anything() {
    for under in [Kind::Cube, Kind::Decor, Kind::Marker] {
        assert!(stands_on(Kind::Cube, under), "on a {under:?}");
    }
}

#[test]
fn decor_marker_support() {
    for tool in [Kind::Decor, Kind::Marker] {
        assert!(stands_on(tool, Kind::Cube), "a {tool:?} on a cube");
        assert!(stands_on(tool, Kind::Decor), "a {tool:?} on a decor");
        assert!(!stands_on(tool, Kind::Marker), "a {tool:?} on a marker");
    }
}
