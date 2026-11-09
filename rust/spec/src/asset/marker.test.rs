// pxl-spec/asset/marker.test.rs

use super::*;
use crate::space::coord::zoom;

/// Markers spread over a few bricks, two to a leaf cell in places.
fn markers() -> Vec<Octant> {
    let z = zoom::UNIT_ZOOM;
    (0..400i32)
        .map(|k| Octant::new(z, [(k * 7919) % 300, (k * 104_729) % 300, (k * 31) % 9]))
        .collect()
}

/// A marker's cell is its parent's child one grid zoom finer, so a cell's
/// count is its children's summed and what is shown only splits as the eye
/// comes nearer; past the marker's own cell it stays there.
#[test]
fn marker_cell_nests() {
    use crate::space::hierarchy::tree::parent_of;
    for at in markers() {
        for cell_zoom in 10..40 {
            let (z, c) = marker_cell_of(cell_zoom, at);
            let (zf, cf) = marker_cell_of(cell_zoom + 1, at);
            if zf == at.zoom() && z == at.zoom() {
                assert_eq!(c, at.at());
            } else {
                assert_eq!((zf, parent_of(cf)), (z + 1, c), "{at:?} at {cell_zoom}");
            }
        }
    }
}
