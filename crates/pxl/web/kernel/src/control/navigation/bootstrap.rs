// pxl-kernel/control/navigation/bootstrap.rs

use pxl_spec::space::adapter::epsg4326::{self, G4326};
use pxl_spec::space::pvf3::core::FRAC_W;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::control::camera::controller::map_center;
use crate::control::navigation::destination::go_to_ground;
use crate::control::navigation::store::map_navigation;
use crate::control::sensor::store::{device_location_store, DevicePoseStatus};
use crate::global::Global;

// Seoul City Hall — the approximate coordinate used when GPS genuinely never
// comes up (permission denied, no signal, timeout, unsupported browser),
// instead of leaving map_center unset forever and never drawing any terrain at
// all. Never touches the sensor store: device_coord stays unset, so the
// location puck naturally has nothing to show and the GPS recenter button
// naturally stays disabled too — both fall out for free from the existing
// state combination.
/// Where the map starts when there is no fix: Seoul City Hall, longitude
/// first as a G4326 is. Three things need it before `map_center` exists — the
/// seed here, the sun and moon the first frames are lit by, and the theme
/// clock's day-or-night — so it is one constant and they read it.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

fn fallback_seed() -> Pvf3 {
    let mut out = Pvf3::default();
    epsg4326::project(&mut out, &FALLBACK_G4326);
    out
}

/// Whether two places are one on the map, across it: within a cell of each
/// other. The map center is derived from the camera, so what is put there
/// comes back to it as near as the camera's frame holds it, not to the bit.
fn is_at(a: &Pvf3, b: &Pvf3) -> bool {
    let cell = 1i64 << FRAC_W;
    (pvf3::X(a) - pvf3::X(b)).abs() < cell && (pvf3::Y(a) - pvf3::Y(b)).abs() < cell
}

struct Bootstrap {
    /// The inputs as last seen.
    last: Option<(Option<Pvf3>, DevicePoseStatus)>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

/// Names where the map starts, the camera put there on the ground once the
/// ground there is known (control/navigation/destination.rs): the
/// bootstrap's turn.
pub fn map_bootstrap_step() {
    let device_coord = map_navigation().device_coord;
    let location_status = device_location_store().status;
    let b = BOOTSTRAP.get(|| Bootstrap { last: None });
    if b.last == Some((device_coord, location_status)) {
        return;
    }
    b.last = Some((device_coord, location_status));
    // prefer the real device position. Only reach for the Seoul fallback
    // once GPS has genuinely given up (ERROR — not merely still
    // LOADING/IDLE) *and* the camera was never placed. A later GPS dropout,
    // after a real fix placed it, intentionally does nothing here and leaves
    // the camera wherever it last was.
    let fallback = fallback_seed();
    let center = map_center();
    let seed = match device_coord {
        Some(device) => device,
        None if center.is_none() && location_status == DevicePoseStatus::Error => fallback,
        None => return,
    };
    // Still at the fallback counts as nothing real yet: a real fix landing
    // after the fallback takes over.
    if center.is_none_or(|c| is_at(&c, &fallback)) {
        go_to_ground(seed);
    }
}
