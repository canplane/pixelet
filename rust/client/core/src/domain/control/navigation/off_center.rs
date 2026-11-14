// pxl-core/domain/control/navigation/off_center.rs

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;

use crate::domain::control::camera::controller;
use crate::domain::control::param::{OFF_CENTER_U, ON_CENTER_U};
use crate::domain::control::sensor::reading::device_location_store;
use crate::domain::query::unknown;
use pxl_global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct OffCenter {
    pub device_coord: Option<Q3>,
    pub is_off_center: bool,
}

static OFF_CENTER: Global<bool> = Global::new();

fn device_coord() -> Option<Q3> {
    device_location_store().location.map(|l| {
        let [lon, lat] = l.lon_lat;
        let at = epsg4326::project(&[lon, lat, l.alt.unwrap_or(0.0)]);
        if l.alt.is_some() {
            at
        } else {
            unknown(&at)
        }
    })
}

/// The device's location, and whether the pivot has wandered off it as of
/// this frame's `off_center_step`.
pub(crate) fn off_center() -> OffCenter {
    OffCenter {
        device_coord: device_coord(),
        is_off_center: *OFF_CENTER.get(|| false),
    }
}

/// Decides once a frame whether the pivot is off the device, past
/// `OFF_CENTER_U` and back within `ON_CENTER_U`.
pub(crate) fn off_center_step() {
    let off = OFF_CENTER.get(|| false);
    // Only the horizontal distance counts. The fix's Z is a GPS altitude and
    // the pivot's Z is a floor, and their mismatch could hold the pivot
    // off-center forever.
    match (controller::pivot(), device_coord()) {
        (Some(pivot), Some(device)) => {
            // X is measured the short way across the antimeridian.
            let [dx, dy, _] = world::between(&pivot, &device).map(q3::to_real);
            let dist = (dx * dx + dy * dy).sqrt();
            if !*off && dist > OFF_CENTER_U {
                *off = true;
            } else if *off && dist < ON_CENTER_U {
                *off = false;
            }
        }
        _ => *off = false,
    }
}
