// pxl-kernel/domain/control/navigation/bootstrap.rs

use pxl_spec::space::coord::adapter::epsg4326;

use crate::domain::view::param::FALLBACK_G4326;
use pxl_spec::space::coord::q3::{FRAC_W, Q3};

use crate::domain::control::camera::controller::pivot;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::navigation::destination::{go_to_ground, pending};
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::navigation::support::unknown_z;
use crate::domain::control::sensor::reading::{device_location_store, DevicePoseStatus};
use xpute_guest::global::Global;

fn fallback_seed() -> Q3 {
    // A place, not a height: it stands on the surface.
    unknown_z(&epsg4326::project(&FALLBACK_G4326))
}

/// Within a cell: the pivot comes back through the camera's floats, not
/// to the bit.
fn is_at(a: &Q3, b: &Q3) -> bool {
    let cell = 1i64 << FRAC_W;
    (i64::from(a[0]) - i64::from(b[0])).abs() < cell
        && (i64::from(a[1]) - i64::from(b[1])).abs() < cell
}

struct Bootstrap {
    last: Option<(Option<Q3>, DevicePoseStatus)>,
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
    let center = pivot();
    let seed = match device_coord {
        Some(device) => device,
        None if center.is_none() && location_status == DevicePoseStatus::Error => fallback,
        None => return,
    };
    // A real fix landing after the fallback takes over; a place asked for
    // and not yet put, a search's or a marker's, is not replaced.
    if center.is_none_or(|c| is_at(&c, &fallback)) && pending().is_none_or(|p| is_at(&p, &fallback))
    {
        go_to_ground(seed, Motion::Snap);
    }
}
