// pxl_core/layer/terrain/ground_snap.rs

use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};

use crate::layer::terrain::elevation::ground_pivot;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;

/// Re-samples terrain elevation under the current map_center and snaps it in,
/// if ground_snap is on. Shared by the tile-arrival retry below and by any
/// one-shot interaction (e.g. the compass's reset-orientation click) that
/// should also leave the view properly grounded instead of only fixing
/// angles/distance.
pub fn reground_map_center() {
    if !settings_store().ground_snap {
        return;
    }
    let Some(map_center) = map_store().map_center else { return };
    let mut probe = PVF3::default();
    pvf3::gather(&mut probe, pvf3::X(&map_center), pvf3::Y(&map_center), 0);
    let (i, f) = (probe.i, probe.f);
    let Some(grounded) = ground_pivot(&mut probe, &PVF3 { i, f }) else { return };
    if grounded != map_center {
        map_store().map_center = Some(grounded);
    }
}
