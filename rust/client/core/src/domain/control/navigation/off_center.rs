// pxl-core/domain/control/navigation/off_center.rs

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;

use crate::domain::control::camera::controller;
use crate::domain::control::sensor::reading::device_location_store;
use crate::domain::control::support::unknown_z;
use xpute_core::global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OffCenter {
    pub device_coord: Option<Q3>,
    pub is_off_center: bool,
}

static OFF_CENTER: Global<bool> = Global::new();

/// The device's location, at its altitude. Where the device gives no
/// altitude, the Z is unknown.
fn device_coord() -> Option<Q3> {
    device_location_store().location.map(|l| {
        let [lon, lat] = l.coords;
        let at = epsg4326::project(&[lon, lat, l.alt.unwrap_or(0.0)]);
        if l.alt.is_some() {
            at
        } else {
            unknown_z(&at)
        }
    })
}

/// The device's location, and whether the pivot has wandered off it as of
/// this turn's `off_center_step`.
pub fn off_center() -> OffCenter {
    OffCenter {
        device_coord: device_coord(),
        is_off_center: *OFF_CENTER.get(|| false),
    }
}

/// Decides once a turn whether the pivot is off the device. The pivot goes
/// off past 10 units and comes back within 3, so a pivot near the edge does
/// not flicker between the two.
pub fn off_center_step() {
    let off = OFF_CENTER.get(|| false);
    // Only the horizontal distance counts. The fix's Z is a GPS altitude and
    // the pivot's Z is a floor, and their mismatch could hold the pivot
    // off-center forever.
    match (controller::pivot(), device_coord()) {
        (Some(pivot), Some(device)) => {
            // X is measured the short way across the antimeridian.
            let [dx, dy, _] = world::between(&pivot, &device).map(q3::to_real);
            let dist = (dx * dx + dy * dy).sqrt();
            if !*off && dist > 10.0 {
                *off = true;
            } else if *off && dist < 3.0 {
                *off = false;
            }
        }
        _ => *off = false,
    }
}
