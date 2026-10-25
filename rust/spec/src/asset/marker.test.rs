// pxl-spec/asset/marker.test.rs

use super::*;

/// Markers spread over a few chunks, two to a leaf cell in places.
fn markers() -> Vec<EntryAt> {
    let z = crate::space::coord::zoom::UNIT_ZOOM;
    (0..400i32).map(|k| EntryAt::new(z, [(k * 7919) % 300, (k * 104_729) % 300, (k * 31) % 9])).collect()
}

/// A marker's cell is its parent's child one grid zoom finer, so a cell's
/// count is its children's summed and what is shown only splits as the eye
/// comes nearer; past the marker's own cell it stays there.
#[test]
fn a_markers_cell_nests_in_the_one_a_grid_zoom_coarser() {
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

/// A marker's cell lies in the chunk drawing its grid's cells, so no cell's
/// count is split between two chunks, wherever the chunk is no finer than
/// the marker.
#[test]
fn a_markers_cell_lies_in_its_chunk() {
    for at in markers() {
        for cell in 19..=at.zoom() + CHUNK_STEPS {
            let (z, c) = marker_cell_of(cell, at);
            let chunk = cell - CHUNK_STEPS;
            assert_eq!(ancestor_of(c, z - chunk), ancestor_of(at.at(), at.zoom() - chunk), "{at:?} at {cell}");
        }
    }
}
