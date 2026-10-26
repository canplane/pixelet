// pxl-spec/space/coord/globe.rs

//! The planet the air is drawn about: its radius, in kilometres and in the
//! map's units, a round of which is its circumference. What is drawn is
//! flat, where the eye's rays find it: nothing is placed on the sphere.

use crate::space::coord::adapter::epsg4326;
use crate::space::coord::world::ROUND;

pub const MERCATOR_RADIUS_KM: f64 = epsg4326::EARTH_RADIUS_M / 1000.0;
pub const MERCATOR_RADIUS_U: f64 = ROUND / core::f64::consts::TAU;
