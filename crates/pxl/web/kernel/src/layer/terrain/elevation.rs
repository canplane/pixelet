// pxl-kernel/layer/terrain/elevation.rs

use pxl_render::mesh::paint::terrain::elevation::{is_water_family_at, sample_ele, WATER_RECESS_CELLS};
use pxl_spec::bake::constant::{PAGE_CELL_SPAN, U_PER_CELL};
use pxl_spec::bake::format::{GroundChannelKey, Terrain};
use pxl_spec::space::indexer::{L1_IDX_IN_L3, L3_IDX, L3_RANGE};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::layer::terrain::store::tile_store;

const L3_RANGE_REAL: f64 = L3_RANGE as f64;

/// The same real height the chunk's mesh and raycast bake
/// (crates/pxl/web/render/src/mesh/paint/terrain/elevation.rs's WATER_RECESS_CELLS), applied here too so this module's own consumers
/// (camera ground-snap, ground_or_raw, any cell-marker probe) agree with the
/// rendered terrain instead of reasoning about the un-recessed height while
/// the mesh they're looking at shows the lowered one. A page's ele and ground
/// are page-level here, so tx/ty index them directly at PAGE_CELL_SPAN stride.
fn sample_ele_grounded(terrain: &Terrain, tx: u32, ty: u32) -> f64 {
    let ele = sample_ele(&terrain.ele, tx, ty);
    if is_water_family_at(&terrain.ground, ty * PAGE_CELL_SPAN + tx) {
        (ele - WATER_RECESS_CELLS) as f64
    } else {
        ele as f64
    }
}

fn page_cell(at: &Pvf3) -> (u32, u32) {
    let page_off = Pvf3 { i: L1_IDX_IN_L3(at.i), f: at.f };
    let tx = (pvf3::to_real(pvf3::X(&page_off)) / U_PER_CELL).floor() as u32;
    let ty = (pvf3::to_real(pvf3::Y(&page_off)) / U_PER_CELL).floor() as u32;
    (tx, ty)
}

/// Returns ground height in lattice/render units.
pub fn get_ele(at: &Pvf3) -> Option<f64> {
    let tile = tile_store().get_tile(L3_IDX(at.i))?;
    let (tx, ty) = page_cell(at);
    Some(sample_ele_grounded(&tile.terrain, tx, ty) * U_PER_CELL)
}

/// Raw baked ground-channel value at this exact cell, one specific channel
/// (LAND/WATER/STRUCTURE/TRANSPORT) — get_ele's own sibling, same tile/tx/ty
/// lookup, just reading `.ground[channel]` instead of sampling `.ele`. Used
/// by object/store.rs's place() to tell whether a terrain-styled pick would
/// actually change anything — painting "land.green_deep" over ground the bake
/// already recorded as GREEN_DEEP land paints nothing new.
pub fn get_ground_type(at: &Pvf3, channel: GroundChannelKey) -> Option<u32> {
    let tile = tile_store().get_tile(L3_IDX(at.i))?;
    let (tx, ty) = page_cell(at);
    Some(tile.terrain.ground.get(channel)[(ty * PAGE_CELL_SPAN + tx) as usize] as u32)
}

/// Fast path for hot per-step loops (e.g. raycasting): looks up elevation from
/// local float coordinates already relative to map_origin (same real "u" units
/// as pvf3::to_real), instead of round-tripping through the full PVF3
/// fixed-point representation on every call.
///
/// `origin_page_idx_xy` must be TO_XY(L3_IDX(map_origin)), precomputed once by
/// the caller outside the loop. map_origin is L4-aligned, which is coarser
/// than (and therefore also aligned to) L3, so page indices compose
/// additively: L3_IDX(local + origin) === L3_IDX(local) + L3_IDX(origin).
pub fn get_ele_at_local(origin_page_idx_xy: IVec, local_x: f64, local_y: f64) -> Option<f64> {
    let local_page_x = (local_x / L3_RANGE_REAL).floor();
    let local_page_y = (local_y / L3_RANGE_REAL).floor();

    // the overwhelmingly common case per-step (a ray mostly stays within the
    // origin page) — skip the gather/add round-trip entirely.
    let page_idx = if local_page_x == 0.0 && local_page_y == 0.0 {
        origin_page_idx_xy
    } else {
        ivec::add(origin_page_idx_xy, ivec::gather(local_page_x as i64, local_page_y as i64, 0))
    };
    let tile = tile_store().get_tile(page_idx)?;

    let page_local_x = local_x - local_page_x * L3_RANGE_REAL;
    let page_local_y = local_y - local_page_y * L3_RANGE_REAL;

    let tx = (page_local_x / U_PER_CELL).floor() as u32;
    let ty = (page_local_y / U_PER_CELL).floor() as u32;
    Some(sample_ele_grounded(&tile.terrain, tx, ty) * U_PER_CELL)
}

pub fn with_ele(out: &mut Pvf3, at: &Pvf3) -> Option<Pvf3> {
    // sample terrain height at the current X/Y position.
    let ele = get_ele(at)?;

    // rebuild absolute PVF3 with ground-relative Z applied.
    *out = *at;
    let grounded_z = pvf3::Z(out) + pvf3::from_real(ele);
    let (x, y) = (pvf3::X(out), pvf3::Y(out));
    pvf3::gather(out, x, y, grounded_z);
    Some(*out)
}

/// Where the camera's pivot sits on the ground at a coordinate's X/Y: the
/// centre of the cube standing on the ground there — the cell the character
/// occupies, the cell a placement's ghost fills — its original Z discarded.
/// None when no elevation is loaded yet, like with_ele.
///
/// A centre, not the top face, because everything the camera pivots on is a
/// cube: a tap on an object pivots on that cube's centre
/// (object/geometry.rs's object_center), and a tap on the ground on the
/// centre of the cube above it (camera/input). A pivot that merely sits on
/// the ground has to agree with those, or the first pan after a tap moves the
/// camera by the half cell between the two. with_ele itself answers the top
/// face, which is what standing things (the puck, decor, a place) are
/// grounded to; this is the one place the camera's own convention is applied.
pub fn ground_pivot(out: &mut Pvf3, at: &Pvf3) -> Option<Pvf3> {
    with_ele(out, at)?;
    let half_cell_fp = pvf3::from_real(U_PER_CELL / 2.0);
    let (x, y, z) = (pvf3::X(out), pvf3::Y(out), pvf3::Z(out));
    pvf3::gather(out, x, y, z + half_cell_fp);
    Some(*out)
}

/// ground_pivot for a coordinate, falling back to the coordinate itself when
/// no elevation is loaded yet.
///
/// The single shared "follow the actual ground" step any map_center-setting
/// interaction should use, instead of each call site hand-rolling its own
/// zero-Z-then-with_ele probe — do_pan and the GPS recenter button used to
/// each build this independently (the recenter button skipped it entirely,
/// setting map_center to the device's raw GPS altitude instead of real
/// terrain height, visibly disagreeing on "where the ground is" depending on
/// which interaction moved the camera).
pub fn ground_or_raw(coord: &Pvf3) -> Pvf3 {
    let mut probe = Pvf3::default();
    pvf3::gather(&mut probe, pvf3::X(coord), pvf3::Y(coord), 0);
    let flat = probe;
    ground_pivot(&mut probe, &flat).unwrap_or(*coord)
}
