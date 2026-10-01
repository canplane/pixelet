// pxl-kernel/control/navigation/bootstrap.rs

use pxl_spec::space::adapter::epsg4326::{self, G4326};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::control::navigation::store::map_navigation;
use crate::control::sensor::store::{device_location_store, DevicePoseStatus};
use crate::global::Global;
use crate::layer::terrain::ground_snap::ground_pivot;
use crate::shell::map_store::map_store;

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

fn is_at(a: &Pvf3, b: &Pvf3) -> bool {
    pvf3::X(a) == pvf3::X(b) && pvf3::Y(a) == pvf3::Y(b)
}

struct Bootstrap {
    /// The inputs as last seen.
    last: Option<(Option<Pvf3>, DevicePoseStatus)>,
    /// A seed set and not yet grounded: retried each turn until the ground
    /// under it has arrived.
    pending: Option<Pvf3>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

/// Seeds and grounds map_center: the bootstrap's turn.
pub fn map_bootstrap_step() {
    let device_coord = map_navigation().device_coord;
    let location_status = device_location_store().status;
    let b = BOOTSTRAP.get(|| Bootstrap { last: None, pending: None });

    if b.last != Some((device_coord, location_status)) {
        b.last = Some((device_coord, location_status));
        b.pending = None;
        // prefer the real device position. Only reach for the Seoul fallback
        // once GPS has genuinely given up (ERROR — not merely still
        // LOADING/IDLE) *and* no center was ever established. A later GPS
        // dropout, after a real fix already seeded map_center, intentionally
        // does nothing here and leaves the camera wherever it last was.
        let fallback = fallback_seed();
        let seed = match device_coord {
            Some(device) => {
                let mut s = Pvf3::default();
                pvf3::gather(&mut s, pvf3::X(&device), pvf3::Y(&device), 0);
                Some(s)
            }
            None if map_store().map_center.is_none() && location_status == DevicePoseStatus::Error => Some(fallback),
            None => None,
        };
        if let Some(seed) = seed {
            // sitting exactly at the fallback seed counts as "nothing real yet"
            // too — otherwise a real fix landing after the fallback kicked in
            // would never actually take over.
            match map_store().map_center {
                None => {
                    map_store().navigate_to(seed);
                    b.pending = Some(seed);
                }
                Some(cur) if is_at(&cur, &fallback) => {
                    map_store().navigate_to(seed);
                    b.pending = Some(seed);
                }
                Some(cur) if is_at(&cur, &seed) => b.pending = Some(seed),
                Some(_) => {}
            }
        }
    }

    let Some(seed) = b.pending else { return };
    let Some(grounded) = ground_pivot(&seed) else { return };
    b.pending = None;
    match map_store().map_center {
        None => map_store().navigate_to(grounded),
        Some(cur) if is_at(&cur, &fallback_seed()) || is_at(&cur, &seed) => map_store().navigate_to(grounded),
        Some(_) => {}
    }
}
