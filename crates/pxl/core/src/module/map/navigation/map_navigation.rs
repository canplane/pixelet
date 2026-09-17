// pxl_core/module/map/navigation/map_navigation.rs

use pxl_shared::spec::space::adapter::epsg4326;
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};

use crate::module::device::pose::store::device_location_store;
use crate::module::map::store::map_store;
use crate::sys::global::Global;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapNavigation {
    /// Device coordinate in canonical world space.
    pub device_coord: Option<PVF3>,
    /// Whether the current center is off from device location.
    pub is_off_center: bool,
}

static OFF_CENTER: Global<bool> = Global::new();

/// The device's location in canonical world space.
fn device_coord() -> Option<PVF3> {
    device_location_store().location.map(|location| {
        let mut out = PVF3::default();
        epsg4326::project(&mut out, &location.coords);
        out
    })
}

/// Device location projected into canonical PVF3 world space, and whether
/// the map centre has wandered off it as of this turn.
pub fn map_navigation() -> MapNavigation {
    MapNavigation {
        device_coord: device_coord(),
        is_off_center: *OFF_CENTER.get(|| false),
    }
}

/// Whether the map centre is off the device, decided once a turn.
pub fn map_navigation_step() {
    let off = OFF_CENTER.get(|| false);
    // Off-center hysteresis over horizontal (X/Y only) distance, not full 3D.
    // device_coord's Z is raw GPS altitude while map_center's Z is baked
    // terrain elevation from a different source (see ground_or_raw) — a real
    // altitude/elevation mismatch of tens of meters could otherwise hold the
    // distance above threshold indefinitely, even sitting exactly on the
    // recentered X/Y spot. Navigation here is inherently horizontal (a top-down
    // map), so only X/Y should decide "did the user wander away".
    match (map_store().map_center, device_coord()) {
        (Some(center), Some(device)) => {
            // X occupies one complete signed lane/world period, so PVF3
            // subtraction gives the short delta across the antimeridian instead
            // of a nearly world-wide raw difference. Y remains ordinary/clamped
            // Mercator space.
            let mut delta = PVF3::default();
            pvf3::sub(&mut delta, &device, &center);
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
