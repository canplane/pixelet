// pxl-kernel/layer/object/collision.rs

use pxl_spec::bake::constant::CELL;
use pxl_spec::space::hierarchy::{share, Scale};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};

use crate::layer::decor::build::scatters_at;
use crate::layer::decor::kind::KINDS;
use crate::layer::object::size::DECOR_SIZE;
use crate::layer::object::store::{holds_entry, objects_at, EntryKind, OccupiedEntry};

/// Every cube sits on its own size's lattice and is at most a cell, so two
/// share space exactly when one's node lies under the other's.
pub fn find_overlapping_object(corner: &Pvf3, size: f64) -> Option<OccupiedEntry> {
    let cube_node = |corner: &Pvf3, size: f64| {
        let side = Scale::of_side(size);
        (side, side.index_of(corner))
    };
    let candidate = cube_node(corner, size);
    objects_at(corner).into_iter().find(|o| share(candidate, cube_node(&Pvf3 { i: o.i, f: o.f }, o.size)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementTool {
    Object,
    Decor,
}

/// The shared check for the placement commit and its hover preview.
pub fn is_placement_blocked(tool: PlacementTool, corner: &Pvf3, object_size: f64) -> bool {
    let size = if tool == PlacementTool::Decor { DECOR_SIZE } else { object_size };
    find_overlapping_object(corner, size).is_some() || tool == PlacementTool::Decor && decor_under(corner)
}

/// Placed or scattered. Decor is stood on nowhere, so nothing is placed on it.
fn decor_under(corner: &Pvf3) -> bool {
    let mut under = *corner;
    pvf3::add(&mut under, corner, &CELL.base_of([0, 0, -1]));
    let placed = objects_at(&under).iter().any(|o| matches!(o.kind, EntryKind::Decor { .. }));
    placed || !holds_entry(&under) && KINDS.iter().filter_map(|k| k.decor_value()).any(|v| scatters_at(v, &under).is_some())
}
