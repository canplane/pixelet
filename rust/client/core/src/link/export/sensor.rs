// pxl-core/link/export/sensor.rs

//! The SENSOR major's commands: the device sensors' readings, failures and
//! permissions, and a request to locate the device.

use super::{applied, bearing, on_earth, refuse_unless};
use crate::domain::control::sensor::device_orientation::{self, OrientationPlatform};
use crate::domain::control::sensor::geolocation::{self, GeolocationPlatform};
use crate::abi::sensor::{GeolocationErrorCode, PermissionAnswer};
use crate::abi::types::Sensor;
use xpute_kit::status::errno::Errno;

pub(super) fn pos(lon: f64, lat: f64, alt: Option<f64>, accuracy: Option<f64>, course: Option<f64>) -> Result<(), Errno> {
    refuse_unless(
        on_earth(lon, lat) && accuracy.is_none_or(|a| a >= 0.0) && bearing(course),
        Errno::EINVAL,
    )?;
    // A fix that comes without an accuracy is taken as having an infinite
    // error.
    geolocation::geolocation_fix(
        lon,
        lat,
        alt,
        accuracy.unwrap_or(f64::INFINITY),
        course,
    );
    Ok(())
}

pub(super) fn err(sensor: Sensor, code: Option<GeolocationErrorCode>, watching: bool) -> Result<(), Errno> {
    match sensor {
        // A failure the platform gave no cause for counts as Other.
        Sensor::Geolocation => {
            geolocation::geolocation_err(
                code.unwrap_or(GeolocationErrorCode::Other),
                watching,
            );
        }
        Sensor::Orient => device_orientation::on_orient_lost(),
    }
    Ok(())
}

pub(super) fn orient(
    webkit_compass_heading: Option<f64>,
    alpha: Option<f64>,
    beta: Option<f64>,
    gamma: Option<f64>,
) -> Result<(), Errno> {
    // DeviceOrientationEvent's ranges, with both ends included, since a
    // reading can lie exactly at an end.
    let within = |v: Option<f64>, half: f64| v.is_none_or(|v| (-half..=half).contains(&v));
    refuse_unless(
        bearing(webkit_compass_heading)
            && bearing(alpha)
            && within(beta, 180.0)
            && within(gamma, 90.0),
        Errno::EINVAL,
    )?;
    device_orientation::orient_event(webkit_compass_heading, alpha, beta, gamma);
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

pub(super) fn platform(geolocation: bool, orient: bool, orient_permission: bool, orient_silence_is_loss: bool) -> Result<(), Errno> {
    *geolocation::platform() = GeolocationPlatform { supported: geolocation };
    *device_orientation::platform() = OrientationPlatform {
        supported: orient,
        needs_permission: orient_permission,
        silence_is_loss: orient_silence_is_loss,
    };
    Ok(())
}
