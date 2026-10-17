// pxl-spec/space/coord/globe.rs

//! The planet the air is drawn about, at the latitude of the sea under the
//! eye: its radius as the map is stretched there, and what the sky reads the
//! planet's map past the terrain by (pxl-render frame/shader/globe.wgsl
//! `off_globe_of`). What is drawn is flat, where the eye's rays find it:
//! nothing is placed on the sphere.

use crate::space::coord::adapter::epsg4326;
use crate::space::coord::world::ROUND;

pub const MERCATOR_RADIUS_KM: f64 = epsg4326::EARTH_RADIUS_M / 1000.0;
pub const MERCATOR_RADIUS_U: f64 = ROUND / core::f64::consts::TAU;

/// e^(y/R) = tan(π/4 + φ/2) and the latitude's sine and cosine, taken once a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Globe {
    pub e0: f64,
    pub sin_lat: f64,
    pub cos_lat: f64,
}

impl Globe {
    pub fn at(lat: f64) -> Globe {
        Globe {
            e0: libm::tan(core::f64::consts::FRAC_PI_4 + lat / 2.0),
            sin_lat: libm::sin(lat),
            cos_lat: libm::cos(lat),
        }
    }

    /// Stretched as the map is there, so the sphere meets the flat map under
    /// the eye.
    pub fn radius_u(&self) -> f64 {
        MERCATOR_RADIUS_U / self.cos_lat
    }
}
