// pxl-kernel/door/apply/sensor.rs

//! SENSOR: the device sensors': their readings, failures and permissions, and the location asked for.

use super::State;
use super::{ask_turn, bearing, on_earth, refuse_unless};
use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::device_orientation::OrientationPlatform;
use crate::domain::control::sensor::geolocation;
use crate::door::cmd as abi;
use crate::turn::world_platform;
use crate::turn::{set_world_platform, Platform};
use xpute_core::ipc::door::Outcome;
use xpute_kit::status::errno::Errno;

pub(super) fn pos(c: abi::SensorPos) -> Outcome {
    refuse_unless(
        on_earth(c.lon, c.lat) && c.accuracy.is_none_or(|a| a >= 0.0) && bearing(c.course),
        Errno::EINVAL,
    )?;
    // An accuracy the device does not give is no accuracy at all.
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
        // A failure the platform named no cause for is Other's.
        abi::Sensor::Geolocation => {
            geolocation::geolocation_err(c.code.unwrap_or(abi::GeolocationErrorCode::Other));
        }
        abi::Sensor::Orientation => device_orientation::on_orient_lost(),
    }
    Ok(Some(0))
}

pub(super) fn orient(c: abi::SensorOrient) -> Outcome {
    // DeviceOrientationEvent's ranges, closed: a reading at an end is still one.
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
                geolocation::on_geolocation_permission_granted(world_platform().geolocation);
            }
        }
        abi::Sensor::Orientation => {
            device_orientation::orient_permission(c.answer, world_platform().orient);
        }
    }
    Ok(Some(0))
}

pub(super) fn locate(s: &mut State) -> Outcome {
    geolocation::start_watching(world_platform().geolocation);
    ask_turn(s)
}

/// What the place's sensors are, which the shell says once the kernel is
/// attached.
pub(super) fn platform(c: abi::SensorPlatform) -> Outcome {
    set_world_platform(Platform {
        geolocation: c.geolocation,
        orient: OrientationPlatform {
            supported: c.orient,
            needs_permission: c.orient_permission,
            silence_is_loss: c.orient_silence_is_loss,
        },
    });
    Ok(Some(0))
}
