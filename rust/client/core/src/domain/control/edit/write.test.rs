// pxl-core/domain/control/edit/write.test.rs

use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube as catalog;
use pxl_spec::space::coord::world;

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, placed, write};

/// Placing a cube over a whole carved cell is redundant only where the
/// ground under the carve is that cube. A cube that was only ever placed is
/// not the ground's.
#[test]
fn place_redundant_only_over_its_own_ground() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    write(
        &mut store,
        [x, y, 4.0],
        GROUND.leaf_cell_zoom(),
        Sentinel::Air as AssetId,
    );
    let world = placed(store);
    let at = world::point(x, y, 4.0);
    let redundant = |cube| place_is_redundant(world, &at, GROUND.leaf_cell_zoom(), cube);
    assert!(redundant(catalog::Ground::Land as AssetId));
    assert!(!redundant(catalog::Ground::Water as AssetId));
    assert!(!redundant(catalog::Object::Red as AssetId));
}

#[test]
fn place_not_redundant_uncarved() {
    let [x, y] = flat_at_five();
    let world = placed(Store::default());
    let at = world::point(x, y, 4.0);
    let cube = catalog::Ground::Land as AssetId;
    assert!(!place_is_redundant(
        world,
        &at,
        GROUND.leaf_cell_zoom(),
        cube
    ));
}

/// A cube that was only ever placed is written as asked, even into a carve
/// of that same cube.
#[test]
fn place_written_over_a_placed_cube() {
    let [x, y] = flat_at_five();
    let red = catalog::Object::Red as AssetId;
    let mut store = Store::default();
    write(&mut store, [x, y, 5.0], GROUND.leaf_cell_zoom() - 1, red);
    write(
        &mut store,
        [x, y, 5.0],
        GROUND.leaf_cell_zoom(),
        Sentinel::Air as AssetId,
    );
    let world = placed(store);
    let at = world::point(x, y, 5.0);
    assert!(!place_is_redundant(
        world,
        &at,
        GROUND.leaf_cell_zoom(),
        red
    ));
}

/// Erasing an entry finer than a leaf cell over ground that fills it writes
/// air: the ground would otherwise fill its cube once it is gone.
#[test]
fn erase_finer_than_a_leaf_leaves_the_hole() {
    let [x, y] = flat_at_five();
    let entry = Octant::holding(GROUND.leaf_cell_zoom() + 1, &world::point(x, y, 4.0));
    assert_eq!(erase_asset_id(entry), Sentinel::Air as AssetId);
}
