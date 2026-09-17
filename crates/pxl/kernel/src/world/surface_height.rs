// pxl-kernel/world/surface_height.rs

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::cell::object_value_is_air;
use pxl_spec::space::indexer::{FROM_L2_IDX, L2_IDX};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, PVF3};
use pxl_spec::space::xy::TO_XY;

use crate::layer::object::store::each_col_entry;
use crate::layer::terrain::elevation::get_ele_at_local;
use crate::world::frame::coord::local_world_to_canonical;

// Two Z values within this of each other count as "flush" — guards the climb
// loop against re-finding the object that just set `surface` on the next pass,
// and the "is this actually resting there" / "is this at or past the hint"
// checks below, against float noise from PVF3 round-trips.
const SAME_SURFACE_EPS: f64 = 1e-6;

/// The z extents of every placed object whose footprint covers a local XY,
/// gathered in one pass over the chunk columns that could hold one.
///
/// One pass rather than a 27-cell neighbourhood query at each step upward: with
/// nothing placed at the XY a probe runs to its 64-cell guard on every call,
/// and the companion asks twice a frame. A placed object is at most a cell across, so
/// one whose footprint covers the point lives in the chunk the point is in or a
/// chunk within a cell of it; those are read once.
fn collect_boxes(map_origin: IVec, local_x: f64, local_y: f64) -> Vec<(f64, f64)> {
    let mut boxes = Vec::new();
    let mut cols: Vec<IVec> = Vec::with_capacity(9);
    for dx in -1..=1 {
        for dy in -1..=1 {
            let probe = [local_x + dx as f64 * U_PER_CELL, local_y + dy as f64 * U_PER_CELL, 0.0];
            let mut corner = PVF3::default();
            local_world_to_canonical(&mut corner, &probe, map_origin);
            let col_idx = L2_IDX(TO_XY(corner.i));
            if cols.contains(&col_idx) {
                continue;
            }
            cols.push(col_idx);
            // The chunk's corner in the local frame; entries are relative to it.
            let local = PVF3 {
                i: ivec::sub(FROM_L2_IDX(col_idx), map_origin),
                f: 0,
            };
            let (bx, by, bz) = (pvf3::to_real(pvf3::X(&local)), pvf3::to_real(pvf3::Y(&local)), pvf3::to_real(pvf3::Z(&local)));
            each_col_entry(
                col_idx,
                &mut |entry| {
                    if object_value_is_air(entry.state) {
                        return;
                    }
                    let (x0, y0) = (bx + entry.lx, by + entry.ly);
                    if local_x < x0 || local_x >= x0 + entry.size || local_y < y0 || local_y >= y0 + entry.size {
                        return;
                    }
                    let z0 = bz + entry.lz;
                    boxes.push((z0, z0 + entry.size));
                },
                false,
            );
        }
    }
    boxes
}

/// Ground height at a local XY point, accounting for placed objects sitting on
/// top of terrain there — not just the raw terrain elevation get_ele_at_local
/// alone reports. The companion needs this: walking onto a placed block (or a
/// stack of them) should stand on top. Thin wrapper over
/// surface_z_from_terrain: the only tile-store-dependent part of this query is
/// the terrain lookup itself, split out so the gap/climb algorithm can be
/// exercised headlessly against synthetic terrain heights.
pub fn get_surface_z_at_local(origin_page_idx_xy: IVec, map_origin: IVec, local_x: f64, local_y: f64, hint_z: f64) -> Option<f64> {
    let terrain_ele = get_ele_at_local(origin_page_idx_xy, local_x, local_y)?;
    Some(surface_z_from_terrain(map_origin, local_x, local_y, terrain_ele, hint_z))
}

/// `hint_z` is roughly where the current navigation target is vertically — used
/// only to pick *which* shelf to land on when the stack at this XY has a gap in
/// it: scans upward from the hint for the lowest object whose top is at or above
/// it, i.e. snaps up past a gap onto the next real platform rather than always
/// defaulting to the first one reachable from the ground. Once landed, still
/// climbs any further objects resting flush on top of that shelf, so the
/// character ends up on the true top of whatever contiguous structure it's
/// standing on. Falls back to climbing from the ground when nothing qualifies
/// above the hint.
pub fn surface_z_from_terrain(map_origin: IVec, local_x: f64, local_y: f64, terrain_ele: f64, hint_z: f64) -> f64 {
    let boxes = collect_boxes(map_origin, local_x, local_y);
    if boxes.is_empty() {
        return terrain_ele;
    }

    // Snap-up: the lowest top at or above the hint, among boxes from the cell
    // below where the search starts — what stepping upward from there found.
    let search_from = hint_z.max(terrain_ele);
    let floor_z = ((search_from / U_PER_CELL).floor() - 1.0) * U_PER_CELL;
    let mut landing: Option<f64> = None;
    for &(min_z, max_z) in &boxes {
        if min_z < floor_z || max_z < hint_z - SAME_SURFACE_EPS {
            continue;
        }
        if landing.is_none_or(|l| max_z < l) {
            landing = Some(max_z);
        }
    }

    let mut surface = landing.unwrap_or(terrain_ele);

    // Climb any further objects resting flush on top of the landing spot. Each
    // pass lifts the surface past at least one box, so it ends within the box
    // count.
    for _ in 0..boxes.len() {
        let mut top: Option<f64> = None;
        for &(min_z, max_z) in &boxes {
            if min_z > surface + SAME_SURFACE_EPS || max_z <= surface + SAME_SURFACE_EPS {
                continue;
            }
            if top.is_none_or(|t| max_z > t) {
                top = Some(max_z);
            }
        }
        let Some(top) = top else { break };
        surface = top;
    }
    surface
}

#[cfg(test)]
#[path = "surface_height.test.rs"]
mod test;
