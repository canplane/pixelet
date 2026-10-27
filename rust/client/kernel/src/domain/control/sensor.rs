// pxl-kernel/domain/control/sensor.rs

//! The GPS and compass, as decisions rather than readings. The kernel watches
//! them only while the shell's world view is up (`WORLD_VIEW`,
//! door/apply.rs), posting `SENSOR_START` and `SENSOR_STOP`, and asks for the
//! location again when the location card is pressed (`INPUT_LOCATE`).

pub mod device_orientation;
pub mod geolocation;
pub mod reading;
