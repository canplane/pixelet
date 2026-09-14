// pxl_core/module/world/object/collision.rs

use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::pvf3::pvf3::PVF3;

use super::geometry::{aabb_overlap, corner_aabb, object_aabb};
use super::size::DECOR_SIZE;
use super::store::{get_objects_near, OccupiedEntry};

/// Whether a candidate box (corner + size) overlaps any existing placed
/// object — real per-axis AABB overlap, which catches cross-size overlap too.
///
/// Only checks objects near `corner` (store.rs's own get_objects_near — the
/// candidate's own cell plus its 26 neighbors), not every placed object in
/// the world — a real object count can be arbitrarily large, and a placement
/// can only ever conflict with something within one cell's own width of it
/// regardless.
pub fn find_overlapping_object(corner: &PVF3, size: f64, map_origin: IVec) -> Option<OccupiedEntry> {
    let candidate = corner_aabb(corner.i, corner.f, size, map_origin);
    get_objects_near(corner, size).into_iter().find(|o| aabb_overlap(&candidate, &object_aabb(o, map_origin)))
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
pub fn is_placement_blocked(tool: PlacementTool, corner: &PVF3, object_size: f64, map_origin: IVec) -> bool {
    let size = if tool == PlacementTool::Decor { DECOR_SIZE } else { object_size };
    find_overlapping_object(corner, size, map_origin).is_some()
}
