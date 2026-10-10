// pxl-kernel/control/interaction/collision.rs

use pxl_spec::space::coord::pvf3::pvf3::{self, Pvf3};
use pxl_spec::space::hierarchy::tree::share;
use pxl_spec::space::layer::lattice::GROUND;

use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{Block, Entry};
use crate::query::World;
use pxl_spec::block::decor::DECORS;
use pxl_spec::space::coord::aabb::Aabb;
use pxl_spec::space::coord::zoom::{base_of, index_of, of_side, units};

/// Every cube sits on its own size's lattice, so two share space exactly
/// when one's node lies under the other's.
pub fn find_overlapping_object(corner: &Pvf3, size: f64) -> Option<Entry> {
    let side = of_side(size);
    let candidate = (side, index_of(side, corner));
    World::now().placed_in(corner).into_iter().find(|o| share(candidate, (o.zoom, o.at)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTool {
    Object,
    /// A decor or a marker: one cell, and nothing stands on it.
    Instance,
}

/// The shared check for the placement commit and its hover preview.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Pvf3, object_size: f64) -> bool {
    let size = if tool == PlacementTool::Instance { DECOR_SIZE } else { object_size };
    find_overlapping_object(corner, size).is_some() || tool == PlacementTool::Instance && instance_under(corner)
}

/// Placed or scattered. An instance is stood on nowhere, so nothing is placed on it.
fn instance_under(corner: &Pvf3) -> bool {
    let under = pvf3::add(corner, &base_of(GROUND.cell(), [0, 0, -1]));
    let world = World::now();
    let placed = world.placed_in(&under).iter().any(|o| matches!(o.block(), Block::Decor { .. } | Block::Marker));
    placed || !world.written_in(&under) && DECORS.iter().any(|k| world.grown(k, &under).is_some())
}

/// The center of the cell a hover anchor (a min corner) points at: where a
/// tap recenters, which is where the ghost box is drawn.
pub fn anchor_center(anchor: &Pvf3) -> Pvf3 {
    let half = pvf3::from_real(units(GROUND.cell()) / 2.0);
    pvf3::gather(pvf3::X(anchor) + half, pvf3::Y(anchor) + half, pvf3::Z(anchor) + half)
}

/// In the world's doubles, the space the pick works in.
pub fn object_aabb(obj: &Entry) -> Aabb {
    let min = crate::view::frame::coord::to_world(&obj.corner());
    let max = min.map(|v| v + obj.size());
    Aabb { min, max }
}
