// pxl-core/domain/control/sensor.rs

//! The core's decisions about the GPS and the compass; their readings are
//! in `reading`. The core watches the sensors only while a surface draws the
//! world, from `SCENE_DRAW` until `GPU_SURFACE_CLOSE` (door/apply.rs), and
//! posts `SENSOR_START` and `SENSOR_STOP` for them. It asks for the location
//! again when the location card is pressed (`SENSOR_LOCATE`).

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

/// The sensors as the UI reports them once the core is attached. Until
/// then the place has no sensor.
pub fn platform() -> &'static mut Platform {
    PLATFORM.get(Platform::default)
}

/// Called when the world's surface opens. It starts the location where the
/// platform has one, and asks for the heading where the platform gives one.
pub fn sensors_open() {
    geolocation_open();
    request_orient();
}

pub fn sensors_close() {
    geolocation_close();
    orient_close();
}
