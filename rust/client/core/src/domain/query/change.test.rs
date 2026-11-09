// pxl-core/domain/query/change.test.rs

use super::*;
use crate::sys;
use pxl_spec::space::layer::lattice::GROUND;

const COLUMNS: [[f64; 2]; 2] = [[0.5, 0.5], [1.5, 1.5]];

/// Whether a kept answer, exact or not, survives a turn whose one change is
/// the ground node at `zoom` over `COLUMNS`.
fn kept(exact: bool, zoom: Zoom) -> bool {
    changes().ground = vec![Quadrant::new(zoom, [0, 0])];
    changes().written.clear();
    let mut memo = Memo(Some((0u32, COLUMNS, exact)));
    memo.forget_touched_at_leaf(|&exact| exact);
    memo.0.is_some()
}

#[test]
fn a_coarser_node_leaves_an_exact_answer_alone() {
    let _serial = sys::serial();
    let leaf = GROUND.leaf_cell_zoom();
    let tile = leaf - GROUND.tile_cell_shamt();
    assert!(!kept(true, tile), "the leaf's tile drops it");
    assert!(
        !kept(true, clipmap::bricks_zoom(leaf)),
        "the leaf's brick drops it"
    );
    assert!(kept(true, tile - 1), "a coarser node leaves it");
    assert!(
        !kept(false, tile - 1),
        "a coarser node drops an answer not exact"
    );
}

#[test]
fn what_objects_wrote_drops_an_exact_answer() {
    let _serial = sys::serial();
    changes().ground.clear();
    changes().written = vec![Aabb {
        min: [1.0, 1.0, BOUNDS.min[2]],
        max: [2.0, 2.0, BOUNDS.max[2]],
    }];
    let mut memo = Memo(Some((0u32, COLUMNS, true)));
    memo.forget_touched_at_leaf(|&exact| exact);
    assert!(memo.0.is_none());
    changes().written.clear();
}
