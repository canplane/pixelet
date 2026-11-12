// pxl-core/door/life.rs

//! The app's lifecycle, which the core serves (xpute wit/life.wit): hidden,
//! the last moment a save is sure to happen, and shown again.

use crate::domain::control::sensor::device_orientation;
use crate::domain::control::sensor::geolocation;
use crate::domain::layer::object::repository;

/// The objects are written, and the position is no longer watched while
/// nothing shows it.
pub(super) fn hidden() {
    geolocation::geolocation_visibility(false);
    repository::save();
}

pub(super) fn shown() {
    geolocation::geolocation_visibility(true);
    device_orientation::orient_visible();
}
