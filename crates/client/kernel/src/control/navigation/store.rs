// pxl-kernel/control/navigation/store.rs

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};

use crate::control::sensor::store::device_location_store;
use xpute_guest::global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapNavigation {
    pub device_coord: Option<Pvf3>,
    pub is_off_center: bool,
}

static OFF_CENTER: Global<bool> = Global::new();

fn device_coord() -> Option<Pvf3> {
    device_location_store().location.map(|location| epsg4326::project(&location.coords))
}

/// Whether the map center has wandered off the device, as of this turn.
pub fn map_navigation() -> MapNavigation {
    MapNavigation {
        device_coord: device_coord(),
        is_off_center: *OFF_CENTER.get(|| false),
    }
}

/// Whether the map center is off the device, decided once a turn.
pub fn map_navigation_step() {
    let off = OFF_CENTER.get(|| false);
    // Horizontal only: the fix's Z is GPS altitude and the center's is terrain
    // from another source, and their mismatch could hold it off-center forever.
    match (crate::control::camera::controller::map_center(), device_coord()) {
        (Some(center), Some(device)) => {
            // PVF3 X wraps a full world period, so the difference is short across the
            // antimeridian.
            let delta = pvf3::sub(&device, &center);
            let dx = pvf3::to_real(pvf3::X(&delta));
            let dy = pvf3::to_real(pvf3::Y(&device) - pvf3::Y(&center));
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
