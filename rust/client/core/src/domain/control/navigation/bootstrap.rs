// pxl-core/domain/control/navigation/bootstrap.rs

use pxl_spec::space::coord::adapter::epsg4326;

use crate::domain::view::param::FALLBACK_G4326;
use pxl_spec::space::coord::q3::{FRAC_W, Q3};

use crate::domain::control::camera::controller::pivot;
use crate::domain::control::camera::controller::Motion;
use crate::domain::control::navigation::destination::{go_to_ground, pending};
use crate::domain::control::navigation::off_center::off_center;
use crate::domain::query::unknown;
use pxl_global::Global;

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
    /// The fix and whether the camera was placed, as the last step saw them.
    last: Option<(Option<Q3>, bool)>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

pub fn map_bootstrap_step() {
    let device_coord = off_center().device_coord;
    let at = pivot();
    let b = BOOTSTRAP.get(|| Bootstrap { last: None });
    if b.last == Some((device_coord, at.is_some())) {
        return;
    }
    b.last = Some((device_coord, at.is_some()));
    // The camera starts at the fallback whenever it was never placed, without
    // waiting on the location. A later dropout leaves the camera where it
    // was.
    let fallback = fallback_seed();
    let seed = match device_coord {
        Some(device) => device,
        None if at.is_none() => fallback,
        None => return,
    };
    // A real fix that lands after the fallback takes over from it. A place
    // asked for and not yet put, such as a search's or a marker's, is not
    // replaced.
    if at.is_none_or(|c| is_at(&c, &fallback)) && pending().is_none_or(|p| is_at(&p, &fallback)) {
        go_to_ground(seed, Motion::Snap);
    }
}
