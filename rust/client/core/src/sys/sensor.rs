// pxl-core/sys/sensor.rs

//! The three facts of the device's sensors the domain reads, as the UI tells
//! them (sensor's methods, link/export/sensor.rs): the heading (`heading`),
//! the device's position (`pos`) and the location's status
//! (`location_status`). The UI runs the sensors and decides about them.

use crate::abi::sensor::{LocationStatus, Position};
use pxl_global::Global;
use pxl_spec::space::coord::adapter::{epsg3857, epsg4326};
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, Grid};

struct Sensor {
    heading: Option<f64>,
    pos: Option<([f64; 2], Option<f64>)>,
    location_status: LocationStatus,
}

static SENSOR: Global<Sensor> = Global::new();

/// A sensor is idle until the UI tells otherwise.
fn sensor() -> &'static mut Sensor {
    SENSOR.get(|| Sensor {
        heading: None,
        pos: None,
        location_status: LocationStatus::Idle,
    })
}

/// The compass heading, 0-360 degrees, or none while there is none.
pub(crate) fn heading() -> Option<f64> {
    sensor().heading
}

/// The device's longitude and latitude, in degrees, and its altitude in
/// meters above sea level where the device gives one; none without a fix.
pub(crate) fn pos() -> Option<([f64; 2], Option<f64>)> {
    sensor().pos
}

/// The location's status; `LocationStatus::Ready` once a fix came.
pub(crate) fn location_status() -> LocationStatus {
    sensor().location_status
}

pub(crate) fn set_heading(deg: Option<f64>) {
    sensor().heading = deg;
}

/// GPS gives `alt_ellipsoid`, over the ellipsoid. It is kept as an `alt`, over sea level, the datum
/// the terrain uses, by the geoid's height at the base corner of the leaf
/// tile the position falls in.
pub(crate) fn set_pos(at: Option<Position>) {
    sensor().pos = at.map(
        |Position {
             lon,
             lat,
             alt_ellipsoid,
         }| {
            let (x0, y0, _) = GROUND
                .cell_of(LEAF_ZOOM, &epsg4326::project(&[lon, lat, 0.0]))
                .tile()
                .rect();
            let [base_lon, base_lat, _] = epsg4326::from_mercator(&[
                epsg3857::merc_from_u(x0),
                epsg3857::merc_from_u(y0),
                0.0,
            ]);
            (
                [lon, lat],
                alt_ellipsoid.map(|h| h - epsg4326::geoid_m(base_lon, base_lat)),
            )
        },
    );
}

pub(crate) fn set_location_status(status: LocationStatus) {
    sensor().location_status = status;
}

#[cfg(test)]
#[path = "sensor.test.rs"]
mod test;
