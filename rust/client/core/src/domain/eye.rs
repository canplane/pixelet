// pxl-core/domain/eye.rs

//! The eye: the pose the world is seen from. The view takes it from the
//! camera where the two can be told apart (view/frame/pose.rs
//! `view_compose`) and composes its matrices at it; the query takes from it
//! the size a place is asked at, by the place's distance from the eye
//! (query/detail.rs `grid_zoom`).

use pxl_global::Global;
use pxl_spec::space::coord::mat4::Mat4;
use pxl_spec::space::coord::v3::V3;

/// `rot` is a pure rotation: it turns the world's axes into the camera's
/// right, up and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EyePose {
    pub eye: V3,
    pub rot: Mat4,
}

static POSE: Global<Option<EyePose>> = Global::new();

/// The pose the world is seen from: the camera's as the view last took it,
/// which the camera's own may differ from by less than can be told apart on
/// the screen. None until the camera stands somewhere.
pub(crate) fn pose() -> Option<EyePose> {
    *POSE.get(|| None)
}

pub(crate) fn set_pose(pose: Option<EyePose>) {
    *POSE.get(|| None) = pose;
}

/// Where the eye stands. None until the camera stands somewhere.
pub(crate) fn at() -> Option<V3> {
    Some(pose()?.eye)
}
