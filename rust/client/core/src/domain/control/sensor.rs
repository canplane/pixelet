// pxl-core/domain/control/sensor.rs

//! The core's decisions about the GPS and the compass; their readings are
//! in `reading`. The core watches the sensors only while a surface draws the
//! world, from `SCENE_DRAW` until `GPU_SURFACE_CLOSE` (link/export.rs), and
//! posts `SENSOR_START` and `SENSOR_STOP` for them. It asks for the location
//! again when the location card is pressed (`SENSOR_LOCATE`).

pub(crate) mod device_orientation;
pub(crate) mod geolocation;
pub(crate) mod reading;

use crate::domain::control::sensor::device_orientation::{orient_close, request_orient};
use crate::domain::control::sensor::geolocation::{geolocation_close, geolocation_open};

/// Called when the world's surface opens. It starts the location where the
/// platform has one, and asks for the heading where the platform gives one.
pub(crate) fn sensors_open() {
    geolocation_open();
    request_orient();
}

pub(crate) fn sensors_close() {
    geolocation_close();
    orient_close();
}
