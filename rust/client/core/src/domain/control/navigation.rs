// pxl-core/domain/control/navigation.rs

//! Where the camera goes: to a named place (`destination`), after the
//! device (`follow_mode`, `off_center`), or to the fallback start when no
//! location comes (`bootstrap`). Going to a named place ends following the
//! device, so it is asked here, of both.

pub(crate) mod bootstrap;
pub(crate) mod destination;
pub(crate) mod follow_mode;
pub(crate) mod off_center;

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::Q3;

use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::Motion;
use crate::domain::query::unknown;

/// Sends the pivot to the surface at the X and Y of `to`, and stops
/// following the device. A marker's height is its zoom's, coarser than where
/// the pivot stands, so the pivot goes on the surface once that is known
/// exactly.
pub(crate) fn go_on_ground(to: Q3, motion: Motion) {
    follow_mode::set_following(false);
    destination::go_to_ground(unknown(&to), motion);
}

pub(crate) fn go_to_place(to: Q3) {
    go_on_ground(to, Motion::Smooth);
    controller::come_in_to_default();
}

/// Goes to a search's place at `lon`, `lat` degrees. A view far out comes in
/// to the default distance.
pub(crate) fn go_to_deg(lon: f64, lat: f64) {
    go_on_ground(epsg4326::project(&[lon, lat, 0.0]), Motion::Snap);
    controller::come_in_to_default();
}
