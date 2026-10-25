// pxl-spec/asset/marker.rs

//! The marker kind (`spec/spec/asset/marker.json`): drawn flat to the camera
//! from a design, its one shape the pin someone puts up. Where a marker's
//! cell is on the camera grid, and from which cell it shows.

#[path = "marker.spec.rs"]
mod spec;
pub use spec::{MARKER_SCALE_STEPS, MARKER_SHOWN_FROM};

use crate::asset::id::{asset_id, AssetId, Kind};
use crate::bake::constant::EntryAt;
use crate::space::camera::grid::CHUNK_STEPS;
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::tree::ancestor_of;

/// The one marker a placement puts up: the pin, which its shape does not
/// tell apart from another's.
pub const MARKER: AssetId = asset_id(Kind::marker, 0);

/// A mark's cell: 16 pixels of the web map's 256-pixel tile, four zooms
/// finer than the tile, the cell Carto keeps its 14-pixel icons apart in.
pub const MARK_STEPS: Zoom = 4;

/// A marker's cell where the camera grid's cell is at `cell`: its scale on
/// the grid (`asset/marker.json`).
pub const fn marker_cell(cell: Zoom) -> i32 {
    cell as i32 + MARKER_SCALE_STEPS as i32
}

/// The marker's cell holding a marker at `at` where the camera grid's cell
/// is at `cell`, which counts the markers in it as one; the marker's own
/// where it is finer.
pub fn marker_cell_of(cell: Zoom, at: EntryAt) -> (Zoom, [i32; 3]) {
    let z = (marker_cell(cell).max(0) as Zoom).min(at.zoom());
    (z, ancestor_of(at.at(), at.zoom() - z))
}

/// Whether a marker shows where the camera grid's cell is at `cell`: its
/// chunk no coarser than `MARKER_SHOWN_FROM`.
pub const fn marker_shows(cell: Zoom) -> bool {
    cell as i32 - CHUNK_STEPS as i32 >= MARKER_SHOWN_FROM as i32
}

#[cfg(test)]
#[path = "marker.test.rs"]
mod test;
