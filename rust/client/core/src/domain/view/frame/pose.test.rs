// pxl-core/domain/view/frame/pose.test.rs

use super::*;

/// The ray a pick casts is the composed view's ray through the same screen
/// point: from the eye to that point on the near plane under the
/// inverse view-projection.
#[test]
fn pick_ray_matches_view() {
    view_resize(1280.0, 720.0);
    let mut look = mat4::new();
    mat4::look_at(
        &mut look,
        [10.0, -20.0, 30.0],
        [40.0, 15.0, 2.0],
        [0.0, 0.0, 1.0],
    );
    let mut rot = look;
    rot[12..15].fill(0.0);
    let pose = ObserverPose {
        eye: [10.0, -20.0, 30.0],
        rot,
    };
    view_compose(Some(pose));
    for [x, y] in [[0.0, 0.0], [0.7, -0.3], [-1.0, 1.0], [0.25, 0.9]] {
        let (mut origin, mut dir) = ([0.0; 3], [0.0; 3]);
        ray_of(x, y, &mut origin, &mut dir);
        let mut near = [0.0; 3];
        mat4::transform_point(&mut near, &view().inv_world_view_proj, x, y, 1.0);
        let mut composed = [0.0; 3];
        mat4::transform_dir(
            &mut composed,
            &mat4::new(),
            near[0] - origin[0],
            near[1] - origin[1],
            near[2] - origin[2],
        );
        assert_eq!(origin, pose.eye);
        for k in 0..3 {
            assert!(
                (dir[k] - composed[k]).abs() < 1e-9,
                "{x},{y}: {dir:?} against {composed:?}"
            );
        }
    }
}

/// A pose at `eye` turned `rad` about the world's up.
fn pose_at(eye: V3, rad: f64) -> ObserverPose {
    let (s, c) = (libm::sin(rad), libm::cos(rad));
    let mut rot = mat4::new();
    (rot[0], rot[1], rot[4], rot[5]) = (c, s, -s, c);
    ObserverPose { eye, rot }
}

/// The view takes a turn only once it can be told apart on the screen: a
/// smaller turn leaves the view as it was, small turns add up against the
/// pose the view keeps until they pass, and a move of the eye is taken at
/// once with the turn it comes with.
#[test]
fn view_takes_turn_past_step() {
    view_resize(1280.0, 720.0);
    let eye = [10.0, -20.0, 30.0];
    assert!(view_compose(Some(pose_at(eye, 0.0))));
    // Half a CSS pixel at this screen's corner is about 3.4e-4 rad.
    assert!(!view_compose(Some(pose_at(eye, 1e-4))));
    assert_eq!(observer::pose(), Some(pose_at(eye, 0.0)));
    assert!(!view_compose(Some(pose_at(eye, 2e-4))));
    assert!(!view_compose(Some(pose_at(eye, 3e-4))));
    assert!(view_compose(Some(pose_at(eye, 4e-4))));
    assert_eq!(observer::pose(), Some(pose_at(eye, 4e-4)));
    assert!(!view_compose(Some(pose_at(eye, 5e-4))));
    let eye_to = [eye[0] + 1.0, eye[1], eye[2]];
    assert!(view_compose(Some(pose_at(eye_to, 5e-4))));
    assert_eq!(observer::pose(), Some(pose_at(eye_to, 5e-4)));
    assert!(view_compose(None));
    assert!(!view_compose(None));
}
