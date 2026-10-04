// pxl-kernel/shell/ui/compass.rs

//! How the compass shows, and what its reset does.

use crate::abi::cmd::CompassState;
use crate::control::camera::input::dragging;
use crate::control::camera::store::{camera_store, CameraStatus};
use crate::global::Global;

static SHOWN: Global<bool> = Global::new();

/// Following while the map follows the heading. Otherwise shown once turned
/// past a threshold that narrows as the camera pulls back, and kept while the
/// map is in hand, so it never vanishes from under a finger turning it back
/// through north.
pub fn compass_state() -> CompassState {
    let camera = camera_store();
    if camera.is_follow_mode {
        SHOWN.set(false);
        return CompassState::Following;
    }
    let azimuth = camera.intent.azimuth;
    let azimuth_thres = (25.0 / (crate::control::camera::controller::radius() + 2.5)).max(3.0).to_radians();
    let in_hand = dragging() || crate::control::camera::controller::zooming() || camera.camera_status != CameraStatus::Idle;
    let shown = azimuth.abs() > azimuth_thres || (*SHOWN.get(|| false) && in_hand);
    SHOWN.set(shown);
    if shown {
        CompassState::Shown
    } else {
        CompassState::Hidden
    }
}

/// Nothing while following the heading, which would turn it straight back.
pub fn compass_reset() {
    if camera_store().is_follow_mode {
        return;
    }
    crate::control::camera::controller::reset_azimuth();
}
