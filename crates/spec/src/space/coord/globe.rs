// pxl-spec/space/coord/globe.rs

//! The flat map laid back on the planet about the map center. Mercator is
//! stretched and flat far from its center, so the ground stopped meeting the
//! horizon and slid when zooming out. Each point is set on the sphere in the
//! center's east-north-up frame, the identity near the center (as Mapbox's
//! globe does). Near-latitude differences use half-angle sines or expm1,
//! never a difference of cosines, so a unit keeps its unit.

use crate::space::coord::adapter::epsg4326;
use crate::space::coord::v3::V3;
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

    /// Stretched as the map is there, which makes the placement the identity at it.
    pub fn radius_u(&self) -> f64 {
        MERCATOR_RADIUS_U / self.cos_lat
    }

    /// `d` is east and north in map units and up from the center's ground.
    pub fn place(&self, d: V3) -> V3 {
        let (e0, s0, c0) = (self.e0, self.sin_lat, self.cos_lat);
        let r = self.radius_u();
        let dl = d[0] / MERCATOR_RADIUS_U;
        let ey = libm::expm1(d[1] / MERCATOR_RADIUS_U);
        let e = e0 * (1.0 + ey);
        // The gudermannian's half-angle form: 2·atan of the exponentials' difference
        // over one plus their product.
        let dphi = 2.0 * libm::atan(e0 * ey / (1.0 + e * e0));
        let c = 2.0 * e / (1.0 + e * e);
        let h = d[2] * c / c0;
        let rr = r + h;
        let (shl, shp) = (libm::sin(dl / 2.0), libm::sin(dphi / 2.0));
        let haversine = shp * shp + c * c0 * shl * shl;
        [rr * c * libm::sin(dl), rr * (libm::sin(dphi) + 2.0 * c * s0 * shl * shl), h - 2.0 * rr * haversine]
    }
    /// A height does not move the result: the clouds are anchored to the map by it.
    pub fn unplace(&self, p: V3) -> [f64; 2] {
        let (s0, c0) = (self.sin_lat, self.cos_lat);
        let v = [p[0], p[1], p[2] + self.radius_u()];
        let axis = v[1] * c0 + v[2] * s0;
        let meridian = -v[1] * s0 + v[2] * c0;
        let east = v[0];
        let across = libm::hypot(east, meridian);
        let length = libm::hypot(across, axis);
        // tan(π/4 + φ/2) = (1 + sin φ) / cos φ, against the center's.
        let e = (length + axis) / across;
        [MERCATOR_RADIUS_U * libm::atan2(east, meridian), MERCATOR_RADIUS_U * libm::log(e / self.e0)]
    }
}

#[cfg(test)]
#[path = "globe.test.rs"]
mod test;
