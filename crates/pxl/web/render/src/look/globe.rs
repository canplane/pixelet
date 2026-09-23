// pxl-render/look/globe.rs

//! The flat map laid back on the planet about the map centre. The world is
//! placed in Web Mercator's plane, which is honest near its centre and not
//! far from it: north of the centre it is stretched by the secant of the
//! latitude, and it is flat where the planet falls away. Past a few tens of
//! kilometres both show — the ground no longer meets the horizon the sky is
//! drawn with, and a view zooming out sees it slide. So each point is taken
//! back to its latitude and longitude and set down on the sphere in the map
//! centre's east-north-up frame, in the map centre's units: the identity
//! there, to first order, so nothing near the camera moves, and the planet
//! Mercator was drawn from further out — Mapbox's globe, which reprojects the
//! same tiles in its vertex stage.
//!
//! The map centre here is the camera centre: the map centre as the view has
//! eased toward it (the kernel's view.rs, `camera_centre`), which the
//! placement's centre, its latitude and the sky over it are all taken from,
//! in the same tick.
//!
//! The sphere is Web Mercator's own (pxl-spec's EPSG:4326 adapter), which is
//! what makes the map centre's neighbourhood the identity; the atmosphere is
//! drawn over the same one, from the eye's own up on it. globe.wgsl's `on_globe_of` is `Globe::place` in f32,
//! written so that a point a unit from the centre keeps its unit: every
//! difference of two near latitudes is taken as a half-angle sine or an
//! expm1, never as a subtraction of two cosines.

use pxl_spec::space::adapter::{epsg3857, epsg4326};
use pxl_spec::space::v3::V3;

use crate::look::azimuth::turn;

/// Web Mercator's sphere.
pub const MERCATOR_RADIUS_KM: f64 = epsg4326::EARTH_RADIUS / 1000.0;
/// A radian of longitude in world units: Mercator's radius in them.
pub const MERCATOR_RADIUS_U: f64 = epsg4326::EARTH_RADIUS * epsg3857::U_PER_M;

/// The sphere as the map centre sees it, which is all the placement needs of
/// where the map is: the Mercator ordinate's exponential there,
/// e^(y/R) = tan(π/4 + φ/2), and the latitude's sine and cosine. Taken once
/// a frame from the map centre's latitude, by the renderer for its uniforms
/// and by the kernel for its culling and the camera's altitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Globe {
    pub e0: f64,
    pub sin_lat: f64,
    pub cos_lat: f64,
}

impl Globe {
    /// The sphere about a map centre at latitude `lat`, in radians.
    pub fn at(lat: f64) -> Globe {
        Globe {
            e0: (core::f64::consts::FRAC_PI_4 + lat / 2.0).tan(),
            sin_lat: lat.sin(),
            cos_lat: lat.cos(),
        }
    }

    /// The sphere's radius in the map centre's units: Mercator's, stretched
    /// as the map is there, which is what makes the placement the identity at
    /// it.
    pub fn radius_u(&self) -> f64 {
        MERCATOR_RADIUS_U / self.cos_lat
    }

    /// A point `d` from the map centre — east and north in map units, and up
    /// from the centre's ground — where the planet has it, in the centre's
    /// east-north-up frame and units.
    pub fn place(&self, d: V3) -> V3 {
        let (e0, s0, c0) = (self.e0, self.sin_lat, self.cos_lat);
        let r = self.radius_u();
        let dl = d[0] / MERCATOR_RADIUS_U;
        let ey = (d[1] / MERCATOR_RADIUS_U).exp_m1();
        let e = e0 * (1.0 + ey);
        // The latitude's difference from the centre's, by the gudermannian's
        // own half-angle form: 2·atan of the exponentials' difference over one
        // plus their product.
        let dphi = 2.0 * (e0 * ey / (1.0 + e * e0)).atan();
        let c = 2.0 * e / (1.0 + e * e);
        // A height is stretched as the map is at its own latitude; back to
        // the centre's units.
        let h = d[2] * c / c0;
        let rr = r + h;
        let (shl, shp) = ((dl / 2.0).sin(), (dphi / 2.0).sin());
        let haversine = shp * shp + c * c0 * shl * shl;
        [rr * c * dl.sin(), rr * (dphi.sin() + 2.0 * c * s0 * shl * shl), h - 2.0 * rr * haversine]
    }
    /// Where on the flat map a point of the planet is: east and north of the
    /// map centre in map units, for `p` in the centre's east-north-up frame and
    /// units as `place` puts it. Its latitude and longitude are read off the
    /// sphere — the point's bearing from the planet's centre, the radius below
    /// the map centre — and taken to Mercator's ordinates, so a height does
    /// not move it: what the clouds are anchored to the map by
    /// (frame/shader/globe.wgsl's `off_globe_of`).
    pub fn unplace(&self, p: V3) -> [f64; 2] {
        let (s0, c0) = (self.sin_lat, self.cos_lat);
        let v = [p[0], p[1], p[2] + self.radius_u()];
        // Along the planet's axis, toward the equator in the centre's
        // meridian, and east.
        let axis = v[1] * c0 + v[2] * s0;
        let meridian = -v[1] * s0 + v[2] * c0;
        let east = v[0];
        let across = east.hypot(meridian);
        let length = across.hypot(axis);
        // tan(π/4 + φ/2) = (1 + sin φ) / cos φ, against the centre's.
        let e = (length + axis) / across;
        [MERCATOR_RADIUS_U * east.atan2(meridian), MERCATOR_RADIUS_U * (e / self.e0).ln()]
    }

    /// A point of the view frame — the map centre at its origin, turned about
    /// Z by the view's azimuth, whose sine and cosine are `s` and `c` (the
    /// root frame's first column) — where the planet has it: turned back to
    /// the map's own axes, placed, and turned forward again. What the module
    /// measures of the world where it is drawn — a chunk's culling, a badge's
    /// place on the screen — goes through this, as every shader that places
    /// the world goes through globe.wgsl's `on_globe_of`. The pick ray is
    /// walked in the flat map: it reaches some fifty units, where the planet
    /// has parted from the plane by a thousandth of one.
    pub fn place_turned(&self, p: V3, s: f64, c: f64) -> V3 {
        let (mut flat, mut placed) = ([0.0; 3], [0.0; 3]);
        *turn(&self.place(*turn(&p, -s, c, &mut flat)), s, c, &mut placed)
    }
}

#[cfg(test)]
#[path = "globe.test.rs"]
mod test;
