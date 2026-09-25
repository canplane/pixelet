// pxl-kernel/layer/object/collision.rs

use pxl_spec::space::hierarchy::share;
use pxl_spec::space::pvf3::pvf3::Pvf3;

use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{cube_node, objects_at, OccupiedEntry};

/// Whether a candidate cube (corner + size) shares space with any placed
/// object: every cube sits on its own size's lattice and is at most a cell,
/// so two share space exactly when one's node lies under the other's, and
/// one that shares the candidate's is in the candidate's own cell.
pub fn find_overlapping_object(corner: &Pvf3, size: f64) -> Option<OccupiedEntry> {
    let candidate = cube_node(corner, size);
    objects_at(corner).into_iter().find(|o| share(candidate, cube_node(&Pvf3 { i: o.i, f: o.f }, o.size)))
}

/// The tool a placement is made with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTool {
    Object,
    Decor,
}

/// Would placing this tool's own candidate at `corner` right now actually
/// land somewhere already occupied — the one "object size vs decor's own
/// fixed DECOR_SIZE" dispatch both the real placement commit and its own
/// hover preview need.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Pvf3, object_size: f64) -> bool {
    let size = if tool == PlacementTool::Decor { DECOR_SIZE } else { object_size };
    find_overlapping_object(corner, size).is_some()
}
