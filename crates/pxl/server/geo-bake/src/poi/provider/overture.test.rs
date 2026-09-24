// pxl-geo-bake/poi/provider/overture.test.rs

use super::*;

#[test]
fn normalization_preserves_subcell_xyz_and_uses_canonical_region_ownership() {
    let root = xy::parse("132110320123013").unwrap();
    let b = xy::bounds(root);
    for x in [b[0] - 0.0001, b[0], b[0] + 0.0001, b[2] - 0.0001, b[2], b[2] + 0.0001] {
        let x_m = x * epsg3857::M_PER_U;
        let y_m = (b[1] + 12.123456) * epsg3857::M_PER_U;
        let p = position(x_m, y_m, 34.123456);
        assert_eq!(p, [x_m * epsg3857::U_PER_M, y_m * epsg3857::U_PER_M, 34.123456 * epsg3857::Z_SCALE(y_m) * epsg3857::U_PER_M]);
        let belongs = p[0] >= b[0] && p[0] < b[2] && p[1] >= b[1] && p[1] < b[3];
        assert_eq!(xy::key_at(p, xy::ROOT_DEPTH) == Some(root), belongs);
        assert_ne!(p[1] * 256.0, (p[1] * 256.0).floor());
    }
}
