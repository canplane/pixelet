// pxl-spec/block/object.test.rs

use super::*;
use crate::block::cube::{object, sentinel};

#[test]
fn every_cube_round_trips_through_its_leaf_value_and_has_a_color() {
    for &cube in ALL_CUBES.iter() {
        assert_eq!(cube_of(cube_id(cube)), Some(cube));
        assert!(cube_rgb(cube).is_some(), "{cube:?}");
    }
}

#[test]
fn no_sentinel_air_or_decor_is_a_cube() {
    for value in [sentinel::void as u32, sentinel::unknown as u32, object::air as u32] {
        assert_eq!(cube_of(value), None, "{value}");
    }
    for &id in decor_ids.iter() {
        assert_eq!(cube_of(decor_num_id(id)), None, "{id:?}");
    }
}

/// Markers spread over a few chunks, two to a leaf cell in places.
fn markers() -> Vec<EntryAt> {
    let z = crate::space::coord::zoom::UNIT_ZOOM;
    (0..400i64).map(|k| EntryAt::new(z, [(k * 7919) % 300, (k * 104_729) % 300, (k * 31) % 9])).collect()
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
