// pxl-kernel/control/navigation/bootstrap.rs

use pxl_spec::space::adapter::epsg4326::{self, G4326};
use pxl_spec::space::pvf3::core::FRAC_W;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::control::camera::controller::map_center;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::store::{device_location_store, DevicePoseStatus};
use crate::global::Global;

/// Where the map starts with no fix: Seoul City Hall. Also lights the first
/// frames and decides day or night before `map_center` exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

fn fallback_seed() -> Pvf3 {
    let mut out = Pvf3::default();
    epsg4326::project(&mut out, &FALLBACK_G4326);
    out
}

/// Within a cell: the map center comes back through the camera's floats, not
/// to the bit.
fn is_at(a: &Pvf3, b: &Pvf3) -> bool {
    let cell = 1i64 << FRAC_W;
    (pvf3::X(a) - pvf3::X(b)).abs() < cell && (pvf3::Y(a) - pvf3::Y(b)).abs() < cell
}

struct Bootstrap {
    last: Option<(Option<Pvf3>, DevicePoseStatus)>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

pub fn map_bootstrap_step() {
    let device_coord = map_navigation().device_coord;
    let location_status = device_location_store().status;
    let b = BOOTSTRAP.get(|| Bootstrap { last: None });
    if b.last == Some((device_coord, location_status)) {
        return;
    }
    b.last = Some((device_coord, location_status));
    // The fallback only once GPS has given up (Error) and the camera was never
    // placed; a later dropout leaves the camera where it was.
    let fallback = fallback_seed();
    let center = map_center();
    let seed = match device_coord {
        Some(device) => device,
        None if center.is_none() && location_status == DevicePoseStatus::Error => fallback,
        None => return,
    };
    // A real fix landing after the fallback takes over.
    if center.is_none_or(|c| is_at(&c, &fallback)) {
        go_to_ground(seed);
    }
}
