// pxl-core/domain/control/sensor.rs

//! The GPS and compass, as decisions rather than readings. The core watches
//! them only while a surface draws the world (`SCENE_DRAW` until
//! `GPU_SURFACE_CLOSE`, door/apply.rs), posting `SENSOR_START` and `SENSOR_STOP`,
//! and asks for the location again when the location card is pressed
//! (`SENSOR_LOCATE`).

pub mod device_orientation;
pub mod geolocation;
pub mod reading;

use crate::domain::control::sensor::device_orientation::{
    orient_close, request_orient, OrientationPlatform,
};
use crate::domain::control::sensor::geolocation::{geolocation_close, geolocation_open};
use xpute_core::global::Global;

/// Which sensors the place has.
#[derive(Clone, Copy, Default)]
pub struct Platform {
    pub geolocation: bool,
    pub orient: OrientationPlatform,
}

static PLATFORM: Global<Platform> = Global::new();

/// As the UI says once the core is attached; no sensor before it has.
pub fn platform() -> &'static mut Platform {
    PLATFORM.get(Platform::default)
}

/// The world's surface opened: the location where the platform has one, the
/// heading where it gives one.
pub fn sensors_open() {
    geolocation_open();
    request_orient();
}

pub fn sensors_close() {
    geolocation_close();
    orient_close();
}
