// pxl-spec/asset/marker.rs

//! The marker kind (`spec/spec/asset/marker.json5`), drawn flat to the camera from a design;
//! its one shape is the pin someone puts up. The module says where a marker's cell is on the
//! camera grid, from which cell it shows, and whether a text fits its kind's cap, a pin's or a
//! chat line's.

#[path = "marker.spec.rs"]
pub mod spec;
pub use spec::{CHAT_TEXT_CHARS, MARKER_SCALE_STEP, MARKER_SHOWN_FROM, MARKER_TEXT_CHARS};

use crate::asset::id::{AssetId, Kind, asset_id};
use crate::space::coord::zoom::Zoom;
use crate::space::hierarchy::tree::ancestor_of;
use crate::space::layer::octant::Octant;

/// The one marker a placement puts up: the pin, which its shape does not
/// tell apart from another's.
pub const MARKER: AssetId = asset_id(Kind::Marker, 0);

/// A mark's cell: 16 pixels of the web map's 256-pixel tile, four zooms
/// finer than the tile, the cell Carto keeps its 14-pixel icons apart in.
pub(crate) const MARK_STEP_CNT: Zoom = 4;

/// A marker's cell where the camera grid's cell is at `cell_zoom`: its scale on
/// the grid (`asset/marker.json5`).
pub const fn marker_cell_zoom(cell_zoom: Zoom) -> i32 {
    cell_zoom as i32 + MARKER_SCALE_STEP as i32
}

/// The marker's cell holding a marker at `at` where the camera grid's cell
/// is at `cell_zoom`, which counts the markers in it as one; the marker's own
/// where it is finer.
pub fn marker_cell_of(cell_zoom: Zoom, at: Octant) -> (Zoom, [i32; 3]) {
    let z = (marker_cell_zoom(cell_zoom).max(0) as Zoom).min(at.zoom());
    (z, ancestor_of(at.at(), at.zoom() - z))
}

/// Whether `text` holds at most `cap` code points, a pin's text or a chat
/// line held to its kind's cap (`MARKER_TEXT_CHARS`, `CHAT_TEXT_CHARS`):
/// one past it is refused whole, never cut short.
pub fn text_fits(text: &str, cap: u32) -> bool {
    text.chars().count() <= cap as usize
}

/// Whether a marker shows where the camera grid's cell is at `cell_zoom`: no
/// coarser than `MARKER_SHOWN_FROM`.
pub const fn marker_shows(cell_zoom: Zoom) -> bool {
    cell_zoom >= MARKER_SHOWN_FROM as Zoom
}

#[cfg(test)]
#[path = "marker.test.rs"]
mod test;
