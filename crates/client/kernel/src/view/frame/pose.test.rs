// pxl-kernel/view/frame/pose.test.rs

use super::*;

/// A pick's ray from a pose is the composed view's through the same point:
/// the near plane's point under the inverse of the view and projection,
/// from the eye.
#[test]
fn a_ray_from_a_pose_is_the_composed_views() {
    view_resize(1280.0, 720.0, 2.0);
    let mut look = mat4::new();
    mat4::look_at(&mut look, [10.0, -20.0, 30.0], [40.0, 15.0, 2.0], [0.0, 0.0, 1.0]);
    let mut rotation = look;
    rotation[12..15].fill(0.0);
    let pose = ObserverPose { eye: [10.0, -20.0, 30.0], rotation };
    view_compose(Some(pose));
    for [x, y] in [[0.0, 0.0], [0.7, -0.3], [-1.0, 1.0], [0.25, 0.9]] {
        let (mut origin, mut direction) = ([0.0; 3], [0.0; 3]);
        ray_of(pose, x, y, &mut origin, &mut direction);
        let mut near = [0.0; 3];
        mat4::transform_point(&mut near, &view().inv_world_view_proj, x, y, 1.0);
        let mut composed = [0.0; 3];
        mat4::transform_direction(&mut composed, &mat4::new(), near[0] - origin[0], near[1] - origin[1], near[2] - origin[2]);
        assert_eq!(origin, pose.eye);
        for k in 0..3 {
            assert!((direction[k] - composed[k]).abs() < 1e-9, "{x},{y}: {direction:?} against {composed:?}");
        }
    }
}
