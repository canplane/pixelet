// pxl-render/mesh/paint/object.test.rs

use super::*;
use pxl_spec::block::overwrite::voxel;

#[test]
fn the_painter_s_voxels_are_the_catalog_s() {
    let p = Painter::new();
    for v in 0..=u32::from(u16::MAX) {
        assert_eq!(matches!(p.kind(v), Kind::Solid | Kind::Air), voxel(v), "value {v}");
    }
}
