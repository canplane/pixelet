// pxl-core/domain/control/navigation/destination.test.rs

use super::*;
use crate::domain::control::camera::controller::{self, Motion};
use crate::domain::control::camera::orbit;
use crate::domain::eye;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed};
use crate::domain::query::unknown;
use pxl_spec::space::coord::{q3, world};

/// Runs the frames' part of a camera: its step, and the view taking its pose.
fn frames(cnt: u32) {
    for _ in 0..cnt {
        dst_step();
        orbit::step(1.0);
        eye::set_pose(controller::pose());
    }
}

/// The app's start: with no eye and no pivot, a camera sent to a place whose
/// height is not known is put there once the leaf is held, and has an eye
/// from then on; its pivot comes to stand on the floor that eye reads.
#[test]
fn a_camera_sent_somewhere_has_an_eye_and_stands_by_it() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    eye::set_pose(None);
    assert!(controller::pose().is_none(), "no eye yet");
    go_to_ground(unknown(&world::point(x + 0.5, y + 0.5, 0.0)), Motion::Snap);
    dst_step();
    assert_eq!(pending(), None, "the destination is reached");
    assert!(controller::pose().is_some(), "the camera has an eye");
    frames(8);
    let pivot = controller::pivot().expect("a pivot");
    assert_eq!(q3::to_real(i64::from(pivot[2])), over(5.0));
}

/// A place named with a height is reached on the floor nearest that height,
/// read by the eye once the camera has brought it there.
#[test]
fn a_height_picks_the_floor_nearest_it_once_the_eye_is_there() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    eye::set_pose(None);
    go_to_ground(world::point(x + 0.5, y + 0.5, 7.0), Motion::Snap);
    dst_step();
    assert!(pending().is_some(), "not reached before an eye reads there");
    assert!(controller::pose().is_some(), "the camera is over the place");
    frames(8);
    assert_eq!(pending(), None, "reached");
    let pivot = controller::pivot().expect("a pivot");
    assert_eq!(q3::to_real(i64::from(pivot[2])), over(5.0));
}
