// pxl-kernel/control/navigation/bootstrap.rs

use pxl_spec::space::coord::adapter::epsg4326::{self, G4326};
use pxl_spec::space::coord::pvf3::core::FRAC_W;
use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};

use crate::control::camera::controller::focus;
use crate::control::camera::controller::Motion;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::off_center::off_center;
use crate::control::sensor::reading::{device_location_store, DevicePoseStatus};
use xpute_guest::global::Global;

/// Where the focus starts with no fix: Seoul City Hall. Also lights the first
/// frames and decides day or night before `focus` exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

fn fallback_seed() -> Pvf3 {
    epsg4326::project(&FALLBACK_G4326)
}

/// Within a cell: the focus comes back through the camera's floats, not
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
    let device_coord = off_center().device_coord;
    let location_status = device_location_store().pose.status();
    let b = BOOTSTRAP.get(|| Bootstrap { last: None });
    if b.last == Some((device_coord, location_status)) {
        return;
    }
    b.last = Some((device_coord, location_status));
    // The fallback only once GPS has given up (Error) and the camera was never
    // placed; a later dropout leaves the camera where it was.
    let fallback = fallback_seed();
    let center = focus();
    let seed = match device_coord {
        Some(device) => device,
        None if center.is_none() && location_status == DevicePoseStatus::Error => fallback,
        None => return,
    };
    // A real fix landing after the fallback takes over.
    if center.is_none_or(|c| is_at(&c, &fallback)) {
        go_to_ground(seed, Motion::Snap);
    }
}
