// pxl-core/domain/control/navigation/follow_mode.rs

use pxl_spec::space::coord::q3::Q3;
use xpute_kit::math::scalar::wrap;

use crate::domain::cmd::SensorStatus;
use crate::domain::cmd::Tracking;
use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::destination::go_to_ground;
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::param::{HEADING_SMOOTH_ALPHA, SENSOR_GRACE_MS};
use crate::domain::control::sensor::device_orientation::request_orient;
use crate::domain::control::sensor::reading::{device_location_store, device_orient_store};
use crate::sys::sched::edge::wake_at;
use xpute_core::clock::now;
use xpute_core::global::Global;

struct Follow {
    /// Whether the camera follows the device. The controls turn it on and
    /// off, and a dead location turns it off.
    on: bool,
    smoothed_azimuth: Option<f64>,
    was_following: bool,
    last_device: Option<Q3>,
    last_heading: Option<f64>,
    /// When following is given up, and the location status that deadline
    /// was set for (`grace_for`).
    grace_due: Option<f64>,
    grace_for: Option<SensorStatus>,
}

static FOLLOW: Global<Follow> = Global::new();

fn follow() -> &'static mut Follow {
    FOLLOW.get(|| Follow {
        on: false,
        smoothed_azimuth: None,
        was_following: false,
        last_device: None,
        last_heading: None,
        grace_due: None,
        grace_for: None,
    })
}

pub fn following() -> bool {
    follow().on
}

pub fn set_following(on: bool) {
    follow().on = on;
}

pub fn follow_mode_step() {
    let f = follow();
    let is_follow_mode = f.on;
    let heading = device_orient_store().orient.and_then(|o| o.heading);
    let device_coord = off_center().device_coord;

    // Following needs the location to be Ready. A heading only turns the
    // camera where there is one, so a device without a compass still follows
    // where it is.
    let status = device_location_store().state.status;
    let ready = status == SensorStatus::Ready;
    if is_follow_mode && !ready {
        // The deadline restarts whenever the status changes, because Loading
        // to Error is a fresh gap with its own retry ahead.
        if f.grace_for != Some(status) {
            f.grace_for = Some(status);
            f.grace_due = Some(now() + SENSOR_GRACE_MS);
        }
        match f.grace_due {
            Some(due) if now() >= due => {
                f.grace_due = None;
                f.grace_for = None;
                f.on = false;
            }
            Some(due) => wake_at(due),
            None => {}
        }
    } else {
        f.grace_due = None;
        f.grace_for = None;
    }

    // The smoothed azimuth is dropped when following ends, so following
    // again snaps to the heading instead of easing from a stale one.
    let entered = is_follow_mode && !f.was_following;
    if !is_follow_mode {
        f.smoothed_azimuth = None;
    }
    f.was_following = is_follow_mode;
    if !is_follow_mode {
        return;
    }

    // The location is not smoothed here, because the smooth move is its only
    // smoothing stage.
    if let Some(device) = device_coord.filter(|d| entered || Some(*d) != f.last_device) {
        go_to_ground(device, Motion::Smooth);
    }
    f.last_device = device_coord;

    if let Some(heading) = heading.filter(|h| entered || Some(*h) != f.last_heading) {
        let raw_azimuth = heading * (core::f64::consts::PI / 180.0);
        let smoothed = match f.smoothed_azimuth {
            None => raw_azimuth,
            Some(s) => {
                s + wrap(
                    raw_azimuth - s,
                    -core::f64::consts::PI,
                    core::f64::consts::PI,
                ) * HEADING_SMOOTH_ALPHA
            }
        };
        f.smoothed_azimuth = Some(smoothed);
        controller::set_intent(IntentPatch {
            azimuth: Some(smoothed),
            ..Default::default()
        });
    }
    f.last_heading = heading;
}

/// The UI picks which of the three actions a press means from the state it
/// drew the button in, so the press means what the user saw.
pub fn tracking_action(tracking: Tracking) {
    match tracking {
        Tracking::Recenter => {
            // The pivot goes to the floor nearest the fix's altitude, or to the
            // surface where the fix has none. The distance is kept.
            if let Some(device) = off_center().device_coord {
                go_to_ground(device, Motion::Snap);
            }
            set_following(false);
        }
        Tracking::Follow => {
            // The heading is asked for, and the view turns with it once it
            // comes. Without a heading, following keeps only the pivot on the
            // device's location.
            request_orient();
            set_following(true);
        }
        Tracking::Unfollow => {
            set_following(false);
            controller::reset_azimuth();
        }
    }
}
