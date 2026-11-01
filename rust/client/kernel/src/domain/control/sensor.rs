// pxl-kernel/domain/control/sensor.rs

//! The GPS and compass, as decisions rather than readings. The kernel watches
//! them only while the world's surface is open (`SURFACE_OPEN` and
//! `SURFACE_CLOSE`, door/apply.rs), posting `SENSOR_START` and `SENSOR_STOP`,
//! and asks for the location again when the location card is pressed
//! (`SENSOR_LOCATE`).

pub mod device_orientation;
pub mod geolocation;
pub mod reading;

use crate::domain::control::sensor::device_orientation::{
    orient_close, request_orient, OrientationPlatform,
};
use crate::domain::control::sensor::geolocation::{geolocation_close, geolocation_open};

/// The world's surface opened: the location where the platform has one, the
/// heading where it gives one.
pub fn sensors_open(geolocation: bool, orient: OrientationPlatform) {
    geolocation_open(geolocation);
    request_orient(orient);
}

pub fn sensors_close() {
    geolocation_close();
    orient_close();
}
