// pxl-spec/asset/marker.test.rs

use super::*;
use crate::space::coord::zoom;

/// Markers spread over a few bricks, two to a leaf cell in places.
fn markers() -> Vec<EntryAt> {
    let z = zoom::UNIT_ZOOM;
    (0..400i32)
        .map(|k| EntryAt::new(z, [(k * 7919) % 300, (k * 104_729) % 300, (k * 31) % 9]))
        .collect()
}

/// A marker's cell is its parent's child one grid zoom finer, so a cell's
/// count is its children's summed and what is shown only splits as the eye
/// comes nearer; past the marker's own cell it stays there.
#[test]
fn marker_cell_nests() {
    use crate::space::hierarchy::tree::parent_of;
    for at in markers() {
        for cell in 10..40 {
            let (z, c) = marker_cell_of(cell, at);
            let (zf, cf) = marker_cell_of(cell + 1, at);
            if zf == at.zoom() && z == at.zoom() {
                assert_eq!(c, at.at());
            } else {
                assert_eq!((zf, parent_of(cf)), (z + 1, c), "{at:?} at {cell}");
            }
        }
    }
}

/// A marker's cell lies in the window's brick drawing its grid's cells
/// (pxl-spec-core window.rs), so no cell's count is split between two
/// bricks, wherever the brick is no finer than the marker.
#[test]
fn marker_cell_in_brick() {
    use pxl_spec_core::window::{bricks_zoom, BRICK_SHIFT};
    for at in markers() {
        for cell in MARKER_SHOWN_FROM as Zoom..=at.zoom() + BRICK_SHIFT {
            let (z, c) = marker_cell_of(cell, at);
            let brick = bricks_zoom(cell);
            let [x, y, _] = ancestor_of(c, z - brick);
            let [ax, ay, _] = ancestor_of(at.at(), at.zoom() - brick);
            assert_eq!([x, y], [ax, ay], "{at:?} at {cell}");
        }
    }
}
