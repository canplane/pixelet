// pxl-kernel/domain/control/navigation/off_center.rs

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::world;

use crate::domain::control::camera::controller;
use crate::domain::control::navigation::support::unknown_z;
use crate::domain::control::sensor::reading::device_location_store;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OffCenter {
    pub device_coord: Option<Q3>,
    pub is_off_center: bool,
}

static OFF_CENTER: Global<bool> = Global::new();

/// At its altitude; where the device gives none, its Z unknown.
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

/// Whether the pivot has wandered off the device, as of this turn.
pub fn off_center() -> OffCenter {
    OffCenter {
        device_coord: device_coord(),
        is_off_center: *OFF_CENTER.get(|| false),
    }
}

/// Whether the pivot is off the device, decided once a turn.
pub fn off_center_step() {
    let off = OFF_CENTER.get(|| false);
    // Horizontal only: the fix's Z is GPS altitude and the pivot's a floor, and
    // their mismatch could hold it off-center forever.
    match (controller::pivot(), device_coord()) {
        (Some(pivot), Some(device)) => {
            // X the short way across the antimeridian.
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
