// pxl-core/domain/control/navigation/bootstrap.rs

use pxl_spec::space::coord::adapter::epsg4326;

use crate::domain::view::param::FALLBACK_G4326;
use pxl_spec::space::coord::q3::{FRAC_W, Q3};

use crate::abi::ui::SensorStatus;
use crate::domain::control::camera::controller::pivot;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::navigation::destination::{go_to_ground, pending};
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::control::sensor::reading::device_location_store;
use crate::domain::query::unknown;
use xpute_core::global::Global;

fn fallback_seed() -> Q3 {
    // The fallback names a place, not a height, so the pivot stands on the
    // surface there.
    unknown(&epsg4326::project(&FALLBACK_G4326))
}

/// Whether `a` and `b` lie within a cell of each other across the ground.
/// The pivot comes back through the camera's floats, so it does not match to
/// the bit.
fn is_at(a: &Q3, b: &Q3) -> bool {
    let cell = 1i64 << FRAC_W;
    (i64::from(a[0]) - i64::from(b[0])).abs() < cell
        && (i64::from(a[1]) - i64::from(b[1])).abs() < cell
}

struct Bootstrap {
    last: Option<(Option<Q3>, SensorStatus)>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

pub fn map_bootstrap_step() {
    let device_coord = off_center().device_coord;
    let location_status = device_location_store().state.status;
    let b = BOOTSTRAP.get(|| Bootstrap { last: None });
    if b.last == Some((device_coord, location_status)) {
        return;
    }
    b.last = Some((device_coord, location_status));
    // The fallback is used only once the location has given up (Error) and
    // the camera was never placed. A later dropout leaves the camera where it
    // was.
    let fallback = fallback_seed();
    let at = pivot();
    let seed = match device_coord {
        Some(device) => device,
        None if at.is_none() && location_status == SensorStatus::Error => fallback,
        None => return,
    };
    // A real fix that lands after the fallback takes over from it. A place
    // asked for and not yet put, such as a search's or a marker's, is not
    // replaced.
    if at.is_none_or(|c| is_at(&c, &fallback)) && pending().is_none_or(|p| is_at(&p, &fallback)) {
        go_to_ground(seed, Motion::Snap);
    }
}
