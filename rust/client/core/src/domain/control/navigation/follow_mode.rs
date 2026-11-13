// pxl-core/domain/control/navigation/follow_mode.rs

use pxl_spec::space::coord::q3::Q3;
use xpute_kit::math::scalar::wrap;

use crate::abi::overlay::SensorStatus;
use crate::abi::camera::Tracking;
use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::camera::state::IntentPatch;
use crate::domain::control::navigation::destination::go_to_ground;
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::param::{HEADING_SMOOTH_ALPHA, SENSOR_GRACE_MS};
use crate::domain::control::sensor::device_orientation::request_orient;
use crate::domain::control::sensor::reading::{device_location_store, device_orient_store};
use crate::sys::sched::invalidate_at;
use xpute_guest::clock::now;
use pxl_global::Global;

struct Follow {
    /// Whether the camera follows the device. The controls turn it on and
    /// off, and a dead location turns it off.
    on: bool,
    /// The azimuth, the device's place and its heading as following last
    /// took them. Each is None while the camera does not follow, so following
    /// again snaps to the device and its heading.
    smoothed_azimuth: Option<f64>,
    last_device: Option<Q3>,
    last_heading: Option<f64>,
    /// The location status a not-ready location's deadline was set for, and
    /// when following is given up.
    grace: Option<(SensorStatus, f64)>,
}

static FOLLOW: Global<Follow> = Global::new();

fn follow() -> &'static mut Follow {
    FOLLOW.get(|| Follow {
        on: false,
        smoothed_azimuth: None,
        last_device: None,
        last_heading: None,
        grace: None,
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
        let due = match f.grace {
            Some((s, due)) if s == status => due,
            _ => {
                let due = now() + SENSOR_GRACE_MS;
                f.grace = Some((status, due));
                due
            }
        };
        if now() >= due {
            f.grace = None;
            f.on = false;
        } else {
            invalidate_at(due);
        }
    } else {
        f.grace = None;
    }

    if !is_follow_mode {
        f.smoothed_azimuth = None;
        f.last_device = None;
        f.last_heading = None;
        return;
    }

    // The location is not smoothed here, because the smooth move is its only
    // smoothing stage.
    if let Some(device) = device_coord.filter(|d| Some(*d) != f.last_device) {
        go_to_ground(device, Motion::Smooth);
    }
    f.last_device = device_coord;

    if let Some(heading) = heading.filter(|h| Some(*h) != f.last_heading) {
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
