// pxl-kernel/control/sensor.rs

//! The device's physical sensors, as decisions rather than readings: GPS and
//! the compass, each behind a store that knows what its permission is, when a
//! watch should run, and what an event is worth acting on. A reading is never
//! taken at face value, because a browser's are not.

pub mod device_orientation;
pub mod geolocation;
pub mod store;
