// pxl-render/mesh/paint/object/adapter.test.rs

use super::*;

#[test]
fn the_painter_s_voxels_are_the_catalog_s() {
    let p = Painter::new();
    for v in 0..=u32::from(u16::MAX) {
        assert_eq!(matches!(p.kind(v), Kind::Solid | Kind::Air), object_value_is_voxel(v), "value {v}");
    }
}
