// pxl-core/domain/control/navigation/location.rs

//! The user's location as the device tells it, and how long tracking
//! outlasts one that is not ready.

use pxl_spec::space::coord::adapter::epsg4326;
use pxl_spec::space::coord::q3::Q3;
use xpute_guest::clock::now;

use crate::abi::sensor::LocationStatus;
use crate::domain::control::param::SENSOR_GRACE_MS;
use crate::domain::query::unknown;
use crate::spec::step::InvalidationReason;
use crate::sys::sched::invalidate_at;
use crate::sys::sensor;

/// The user's location as the device tells it, its height unknown where
/// the fix has no altitude.
pub(crate) fn user_location() -> Option<Q3> {
    sensor::pos().map(|([lon, lat], alt)| {
        let at = epsg4326::project(&[lon, lat, alt.unwrap_or(0.0)]);
        if alt.is_some() { at } else { unknown(&at) }
    })
}

/// The location status a not-ready location's deadline was set for, and
/// when tracking is given up; none while the location is ready.
pub(crate) type Grace = Option<(LocationStatus, f64)>;

/// Whether the tracked location is lost, asked each frame it is tracked,
/// with tracking's `grace`. Tracking needs the location to be Ready, and
/// outlasts one that is not by a grace: a gap the location recovers from
/// keeps tracking, and a dead location ends it. The deadline restarts
/// whenever the status changes, because Loading to Error is a fresh gap with
/// its own retry ahead.
pub(crate) fn lost(grace: &mut Grace) -> bool {
    let status = sensor::location_status();
    if status == LocationStatus::Ready {
        *grace = None;
        return false;
    }
    let due = match *grace {
        Some((was, due)) if was == status => due,
        _ => now() + SENSOR_GRACE_MS,
    };
    *grace = Some((status, due));
    if now() < due {
        invalidate_at(due, InvalidationReason::Pending);
    }
    now() >= due
}
