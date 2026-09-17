// pxl_core/module/world/chunk_raycast.test.rs

use super::*;

#[test]
fn the_slab_test_names_the_entered_face() {
    let s = ray_aabb(&[0.5, 0.5, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0]).unwrap();
    assert_eq!((s.t, s.axis, s.sign), (4.0, 2, 1.0));
    assert!(ray_aabb(&[2.0, 0.5, 5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0]).is_none());
    // Behind the origin.
    assert!(ray_aabb(&[0.5, 0.5, -5.0], &[0.0, 0.0, -1.0], &[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0]).is_none());
}
