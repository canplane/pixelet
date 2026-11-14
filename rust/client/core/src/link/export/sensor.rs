// pxl-core/link/export/sensor.rs

//! The sensor interface's methods: the sensors the platform has, their
//! readings, failures and permissions, and a request to locate the device.

use super::{applied, bearing, on_earth, refuse_unless};
use crate::abi::sensor::{GeolocationErrorCode, PermissionAnswer};
use crate::abi::types::Sensor;
use crate::domain::control::navigation::follow_mode::following;
use crate::domain::control::sensor::device_orientation::{self, OrientationPlatform};
use crate::domain::control::sensor::geolocation::{self, GeolocationPlatform};
use xpute_kit::status::errno::Errno;

pub(super) fn pos(
    lon: f64,
    lat: f64,
    alt: Option<f64>,
    accuracy: Option<f64>,
    course: Option<f64>,
) -> Result<(), Errno> {
    refuse_unless(
        on_earth(lon, lat) && accuracy.is_none_or(|a| a >= 0.0) && bearing(course),
        Errno::EINVAL,
    )?;
    // A fix that comes without an accuracy is taken as having an infinite
    // error. The location indicator stands at the position, so a fix that
    // moves it asks for a frame.
    let moved =
        geolocation::geolocation_fix(lon, lat, alt, accuracy.unwrap_or(f64::INFINITY), course);
    if moved {
        return applied();
    }
    Ok(())
}

pub(super) fn orient_unavailable() -> Result<(), Errno> {
    device_orientation::orient_unavailable();
    Ok(())
}

pub(super) fn err(
    sensor: Sensor,
    code: Option<GeolocationErrorCode>,
    watching: bool,
) -> Result<(), Errno> {
    match sensor {
        // A failure the platform gave no cause for counts as `Other`. A
        // cleared location moves the location indicator, and while following
        // a failed one starts follow mode's grace (`follow_mode_step`), whose
        // deadline then wakes its own frame.
        Sensor::Geolocation => {
            let cleared =
                geolocation::geolocation_err(code.unwrap_or(GeolocationErrorCode::Other), watching);
            if cleared || following() {
                return applied();
            }
        }
        Sensor::Orient => device_orientation::on_orient_lost(),
    }
    Ok(())
}

pub(super) fn orient(webkit_compass_heading: Option<f64>, alpha: Option<f64>) -> Result<(), Errno> {
    refuse_unless(
        bearing(webkit_compass_heading) && bearing(alpha),
        Errno::EINVAL,
    )?;
    // The heading turns the camera only while it follows the device, so a
    // changed heading asks for a frame only then.
    if device_orientation::orient_event(webkit_compass_heading, alpha) && following() {
        return applied();
    }
    Ok(())
}

pub(super) fn permission(sensor: Sensor, answer: PermissionAnswer) -> Result<(), Errno> {
    match sensor {
        Sensor::Geolocation => {
            if answer == PermissionAnswer::Granted {
                geolocation::on_geolocation_permission_granted();
            }
        }
        Sensor::Orient => {
            device_orientation::orient_permission(answer);
        }
    }
    Ok(())
}

pub(super) fn locate() -> Result<(), Errno> {
    geolocation::start_watching();
    applied()
}

pub(super) fn platform(
    geolocation: bool,
    orient: bool,
    orient_permission: bool,
    orient_silence_is_loss: bool,
) -> Result<(), Errno> {
    *geolocation::platform() = GeolocationPlatform {
        supported: geolocation,
    };
    *device_orientation::platform() = OrientationPlatform {
        supported: orient,
        needs_permission: orient_permission,
        silence_is_loss: orient_silence_is_loss,
    };
    Ok(())
}
