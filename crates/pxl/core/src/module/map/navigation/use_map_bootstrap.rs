// pxl_core/module/map/navigation/use_map_bootstrap.rs

use pxl_shared::spec::space::adapter::epsg4326::{self, G4326};
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};

use super::use_map_navigation::use_map_navigation;
use crate::global::Global;
use crate::module::device::pose::store::{use_device_location_store, DevicePoseStatus};
use crate::module::map::store::use_map_store;
use crate::module::world::terrain::elevation::ground_pivot;

// Seoul City Hall — the approximate coordinate used when GPS genuinely never
// comes up (permission denied, no signal, timeout, unsupported browser),
// instead of leaving map_center unset forever and never drawing any terrain at
// all. Never touches the device/pose store: device_coord stays unset, so the
// location puck naturally has nothing to show and the GPS recenter button
// naturally stays disabled too — both fall out for free from the existing
// state combination.
/// Where the map starts when there is no fix: Seoul City Hall, longitude
/// first as a G4326 is. Three things need it before `map_center` exists — the
/// seed here, the sun and moon the first frames are lit by, and the theme
/// clock's day-or-night — so it is one constant and they read it.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

fn fallback_seed() -> PVF3 {
    let mut out = PVF3::default();
    epsg4326::project(&mut out, &FALLBACK_G4326);
    out
}

fn is_at(a: &PVF3, b: &PVF3) -> bool {
    pvf3::X(a) == pvf3::X(b) && pvf3::Y(a) == pvf3::Y(b)
}

struct Bootstrap {
    /// The effect's dependencies as they were last run.
    last: Option<(Option<PVF3>, DevicePoseStatus)>,
    /// A seed set and not yet grounded: retried each turn until the ground
    /// under it has arrived.
    pending: Option<PVF3>,
}

static BOOTSTRAP: Global<Bootstrap> = Global::new();

/// Seeds and grounds map_center: the bootstrap's turn.
pub fn map_bootstrap_step() {
    let device_coord = use_map_navigation().device_coord;
    let location_status = use_device_location_store().status;
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
                let mut s = PVF3::default();
                pvf3::gather(&mut s, pvf3::X(&device), pvf3::Y(&device), 0);
                Some(s)
            }
            None if use_map_store().map_center.is_none() && location_status == DevicePoseStatus::Error => Some(fallback),
            None => None,
        };
        if let Some(seed) = seed {
            // sitting exactly at the fallback seed counts as "nothing real yet"
            // too — otherwise a real fix landing after the fallback kicked in
            // would never actually take over.
            match use_map_store().map_center {
                None => {
                    use_map_store().set_map_center(seed);
                    b.pending = Some(seed);
                }
                Some(cur) if is_at(&cur, &fallback) => {
                    use_map_store().set_map_center(seed);
                    b.pending = Some(seed);
                }
                Some(cur) if is_at(&cur, &seed) => b.pending = Some(seed),
                Some(_) => {}
            }
        }
    }

    let Some(seed) = b.pending else { return };
    let mut probe = seed;
    let Some(grounded) = ground_pivot(&mut probe, &seed) else { return };
    b.pending = None;
    match use_map_store().map_center {
        None => use_map_store().set_map_center(grounded),
        Some(cur) if is_at(&cur, &fallback_seed()) || is_at(&cur, &seed) => use_map_store().set_map_center(grounded),
        Some(_) => {}
    }
}
