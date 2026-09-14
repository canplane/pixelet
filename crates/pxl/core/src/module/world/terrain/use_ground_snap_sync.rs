// pxl_core/module/world/terrain/use_ground_snap_sync.rs

use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};

use super::elevation::ground_pivot;
use super::store::tile_pages;
use crate::app::setting::store::use_settings_store;
use crate::global::Global;
use crate::module::map::store::use_map_store;

/// Re-samples terrain elevation under the current map_center and snaps it in,
/// if ground_snap is on. Shared by the tile-arrival retry below and by any
/// one-shot interaction (e.g. the compass's reset-orientation click) that
/// should also leave the view properly grounded instead of only fixing
/// angles/distance.
pub fn reground_map_center() {
    if !use_settings_store().ground_snap {
        return;
    }
    let Some(map_center) = use_map_store().map_center else { return };
    let mut probe = PVF3::default();
    pvf3::gather(&mut probe, pvf3::X(&map_center), pvf3::Y(&map_center), 0);
    let (i, f) = (probe.i, probe.f);
    let Some(grounded) = ground_pivot(&mut probe, i, f) else { return };
    if grounded != map_center {
        use_map_store().set_map_center(grounded);
    }
}

static SEEN: Global<u32> = Global::new();

/// Retries grounding map_center whenever new terrain data arrives.
///
/// The pan handler grounds map_center on every drag step, but if a drag lands
/// the center in an area whose tile hadn't finished loading yet, that attempt
/// falls back to the stale pre-drag height and nothing else re-checks it once
/// the tile does arrive — the center can stay ungrounded indefinitely.
pub fn ground_snap_sync_step() {
    let revision = tile_pages().revision();
    let seen = SEEN.get(|| u32::MAX);
    if *seen == revision {
        return;
    }
    *seen = revision;
    reground_map_center();
}
