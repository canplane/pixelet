// pxl-kernel/layer/terrain/elevation.rs

use pxl_render::mesh::paint::terrain::elevation::{ground_ele, GroundLanes};
use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::format::{GroundChannelKey, Terrain};
use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::pyramid::{Cell, LevelPage, GROUND, TOP_LEVEL};
use pxl_spec::space::xy::TO_XY;

use crate::world::pyramid::Layer as _;

/// The height the chunk's mesh and its raycast are built from, asked of the
/// one query that answers it (crates/pxl/web/render/src/mesh/paint/terrain/elevation.rs's
/// ground_ele), so this module's own consumers — camera ground-snap,
/// ground_or_raw, any cell-marker probe — agree with the terrain on screen
/// instead of reasoning about a height it does not show.
fn sample_ele_grounded(terrain: &Terrain, cell: Cell) -> f64 {
    let [tx, ty] = cell.in_page();
    ground_ele(&terrain.ele, Some(GroundLanes::of(&terrain.ground)), tx as i32, ty as i32) as f64
}

/// The ground's height at `at` as the finest resident page draws it, in
/// units: the leaf's where it is held, else the nearest coarser page's own
/// raster at that point — the surface a view there is standing over, not a
/// page's extremes. None where no page over it is held.
///
/// The one question every reader of the terrain's height asks — the
/// camera's clearance, a character's footing, a marker, the map center — so
/// that none of them stands on a height the others do not: before a leaf
/// comes, all of them stand on the page over it.
pub fn ground_at(at: &Pvf3) -> Option<f64> {
    let leaf = TO_XY(L3_IDX(at.i));
    (0..=TOP_LEVEL).find_map(|level| {
        let tile = Terrain::pyramid().get(LevelPage::over(level, leaf))?;
        Some(sample_ele_grounded(tile, GROUND.cell_of(level, at)) * U_PER_CELL)
    })
}

/// Raw baked ground-channel value at this exact z16 cell, one specific
/// channel (LAND/WATER/STRUCTURE/TRANSPORT), from the leaf page: a type is a
/// z16 cell's, which a coarser page's vote does not say. Used
/// by object/store.rs's place() to tell whether a terrain-styled pick would
/// actually change anything — painting "land.green_deep" over ground the bake
/// already recorded as GREEN_DEEP land paints nothing new.
pub fn get_ground_type(at: &Pvf3, channel: GroundChannelKey) -> Option<u32> {
    let cell = GROUND.cell_of(0, at);
    let tile = Terrain::pyramid().get(cell.page())?;
    Some(tile.ground.channel(cell.index() as usize, channel) as u32)
}

pub fn with_ele(out: &mut Pvf3, at: &Pvf3) -> Option<Pvf3> {
    // sample terrain height at the current X/Y position.
    let ele = ground_at(at)?;

    // rebuild absolute PVF3 with ground-relative Z applied.
    *out = *at;
    let grounded_z = pvf3::Z(out) + pvf3::from_real(ele);
    let (x, y) = (pvf3::X(out), pvf3::Y(out));
    pvf3::gather(out, x, y, grounded_z);
    Some(*out)
}

/// Where the camera's pivot sits on the ground at a coordinate's X/Y: the
/// center of the cube standing on the ground there — the cell the character
/// occupies, the cell a placement's ghost fills — its original Z discarded.
/// None when no elevation is loaded yet, like with_ele.
///
/// A center, not the top face, because everything the camera pivots on is a
/// cube: a tap on an object pivots on that cube's center
/// (object/geometry.rs's object_center), and a tap on the ground on the
/// center of the cube above it (camera/input). A pivot that merely sits on
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
/// The one "follow the actual ground" step every map_center-setting
/// interaction takes — a pan, the GPS recenter — so that all of them agree
/// on where the ground is: the coordinate probed at Z = 0 and lifted to the
/// terrain's height, or the coordinate as given where no terrain is loaded.
/// A recenter that set map_center to the device's raw GPS altitude instead
/// would put the camera at a different ground than a pan does.
pub fn ground_or_raw(coord: &Pvf3) -> Pvf3 {
    let mut probe = Pvf3::default();
    pvf3::gather(&mut probe, pvf3::X(coord), pvf3::Y(coord), 0);
    let flat = probe;
    ground_pivot(&mut probe, &flat).unwrap_or(*coord)
}
