// pxl-core/door/apply/sensor.rs

//! The SENSOR major's commands: the device sensors' readings, failures and
//! permissions, and a request to locate the device.

use super::State;
use super::{ask_turn, bearing, on_earth, refuse_unless};
use crate::domain::control::sensor;
use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::device_orientation::OrientationPlatform;
use crate::domain::control::sensor::geolocation;
use crate::domain::control::sensor::Platform;
use crate::door::cmd as abi;
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn pos(c: abi::SensorPos) -> Outcome {
    refuse_unless(
        on_earth(c.lon, c.lat) && c.accuracy.is_none_or(|a| a >= 0.0) && bearing(c.course),
        Errno::EINVAL,
    )?;
    // A fix that comes without an accuracy is taken as having an infinite
    // error.
    geolocation::geolocation_fix(
        c.lon,
        c.lat,
        c.alt,
        c.accuracy.unwrap_or(f64::INFINITY),
        c.course,
    );
    Ok(Some(0))
}

pub(super) fn err(c: abi::SensorErr) -> Outcome {
    match c.sensor {
        // A failure the platform gave no cause for counts as Other.
        abi::Sensor::Geolocation => {
            geolocation::geolocation_err(c.code.unwrap_or(abi::GeolocationErrorCode::Other));
        }
        abi::Sensor::Orientation => device_orientation::on_orient_lost(),
    }
    Ok(Some(0))
}

pub(super) fn orient(c: abi::SensorOrient) -> Outcome {
    // DeviceOrientationEvent's ranges, with both ends included, since a
    // reading can lie exactly at an end.
    let within = |v: Option<f64>, half: f64| v.is_none_or(|v| (-half..=half).contains(&v));
    refuse_unless(
        bearing(c.webkit_compass_heading)
            && bearing(c.alpha)
            && within(c.beta, 180.0)
            && within(c.gamma, 90.0),
        Errno::EINVAL,
    )?;
    device_orientation::orient_event(c.webkit_compass_heading, c.alpha, c.beta, c.gamma);
    Ok(Some(0))
}

pub(super) fn permission(c: abi::SensorPermission) -> Outcome {
    match c.sensor {
        abi::Sensor::Geolocation => {
            if c.answer == abi::PermissionAnswer::Granted {
                geolocation::on_geolocation_permission_granted();
            }
        }
        abi::Sensor::Orientation => {
            device_orientation::orient_permission(c.answer);
        }
    }
    Ok(Some(0))
}

pub(super) fn locate(s: &mut State) -> Outcome {
    geolocation::start_watching();
    ask_turn(s)
}

/// Which sensors the place has. The UI sends this once the core is
/// attached.
pub(super) fn platform(c: abi::SensorPlatform) -> Outcome {
    *sensor::platform() = Platform {
        geolocation: c.geolocation,
        orient: OrientationPlatform {
            supported: c.orient,
            needs_permission: c.orient_permission,
            silence_is_loss: c.orient_silence_is_loss,
        },
    };
    Ok(Some(0))
}
