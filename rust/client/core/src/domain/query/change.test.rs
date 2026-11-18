// pxl-core/domain/query/change.test.rs

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed};
use pxl_spec::brick;
use pxl_spec::space::coord::world::point;
use pxl_spec::space::layer::lattice::GROUND;

const COLUMNS: [[f64; 2]; 2] = [[0.5, 0.5], [1.5, 1.5]];

/// Whether a kept answer, exact or not, survives a frame whose one change is
/// the ground node at `zoom` over `COLUMNS`.
fn kept(exact: bool, zoom: Zoom) -> bool {
    changes().ground = vec![Quadrant::new(zoom, [0, 0])];
    changes().damage.clear();
    let mut memo = Memo(Some((0u32, COLUMNS, exact)));
    memo.forget_touched_at_leaf(|&exact| exact);
    memo.0.is_some()
}

#[test]
fn a_coarser_node_leaves_an_exact_answer_alone() {
    let leaf = GROUND.leaf_cell_zoom();
    let tile = leaf - GROUND.tile_cell_shamt();
    assert!(!kept(true, tile), "the leaf's tile drops it");
    assert!(
        !kept(true, brick::bricks_zoom(leaf)),
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
    changes().ground.clear();
    changes().damage = vec![Aabb {
        min: [1.0, 1.0, BOUND.min[2]],
        max: [2.0, 2.0, BOUND.max[2]],
    }];
    let mut memo = Memo(Some((0u32, COLUMNS, true)));
    memo.forget_touched_at_leaf(|&exact| exact);
    assert!(memo.0.is_none());
    changes().damage.clear();
}

/// The queries report the columns they read, joined: a support reads its own
/// cell, and a walk reads every cell it passes.
#[test]
fn queries_report_the_columns_they_read() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    *read_rect() = ReadRect::default();
    let cell = units(GROUND.leaf_cell_zoom());
    let at = [x + 0.5 * cell, y + 0.5 * cell];
    let c = at.map(|v| libm::floor(v / cell) * cell);
    world.surface_support(&point(at[0], at[1], 0.0));
    assert_eq!(read_rect().0, Some([c, c.map(|v| v + cell)]));
    let from = point(at[0], at[1], over(5.0));
    world.step_slide(&from, &point(at[0] + 3.0 * cell, at[1], 0.0), &from, true);
    assert_eq!(read_rect().0, Some([c, [c[0] + 4.0 * cell, c[1] + cell]]));
    // A body of known height that does not show reads nothing, however
    // far it goes.
    *read_rect() = ReadRect::default();
    let high = point(at[0], at[1], 1000.0);
    world.step_slide(
        &from,
        &point(at[0] + 3000.0 * cell, at[1], 0.0),
        &high,
        true,
    );
    assert_eq!(read_rect().0, None);
}

/// A memo keeps what a cell's asset answers and asks again when a change
/// touches the cell's column.
#[test]
fn a_memo_keeps_a_cell_until_its_column_changes() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let mut memo = Memo::<u32, Option<u32>>::NONE;
    let at = point(x + 0.5, y + 0.5, 4.5);
    let asked = memo.of(0, || world.asset_of(&at).val());
    assert!(asked.is_some());
    assert!(memo.0.is_some(), "a cell's asset read its column");
    changes().ground.clear();
    changes().damage.clear();
    memo.forget_touched();
    assert!(memo.0.is_some(), "no change touched the column");
    changes().damage = vec![Aabb {
        min: [x, y, BOUND.min[2]],
        max: [x + 1.0, y + 1.0, BOUND.max[2]],
    }];
    memo.forget_touched();
    assert!(memo.0.is_none(), "a write over the column drops it");
    changes().damage.clear();
}
