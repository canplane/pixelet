// pxl-spec/bake/constant.test.rs

#[test]
fn a_level_0_cell_is_the_standard_z16_pixel_exactly() {
    use crate::space::adapter::epsg3857::M_PER_U;
    // The OGC WebMercatorQuad z0 resolution, 256-pixel tiles: the world's
    // width over 256. At z16 it is the metres a level-0 cell spans.
    let z16_m_per_px = 156543.03392804097 / (1u64 << 16) as f64;
    assert_eq!(M_PER_U, z16_m_per_px);
    assert_eq!(crate::space::pyramid::zoom_of_level(0), 15);
    assert_eq!(crate::space::pyramid::zoom_of_level(crate::space::pyramid::TOP_LEVEL), 5);
}
