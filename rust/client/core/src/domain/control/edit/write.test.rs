// pxl-core/domain/control/edit/write.test.rs

use pxl_spec::asset::cube as catalog;
use pxl_spec::asset::AssetId;
use pxl_spec::space::coord::world;

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;

/// Placing a cube over a whole carved cell is redundant only where the
/// ground under the carve is that cube. A cube that was only ever placed is
/// not the ground's.
#[test]
fn place_redundant_only_over_its_own_ground() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    write(&mut store, [x, y, 4.0], GROUND.leaf_cell_zoom(), AIR);
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 4.0);
    let redundant = |cube| place_val_is_redundant(world, &at, GROUND.leaf_cell_zoom(), cube);
    assert!(redundant(catalog::Ground::land as AssetId));
    assert!(!redundant(catalog::Ground::water as AssetId));
    assert!(!redundant(catalog::Object::red as AssetId));
}

#[test]
fn place_not_redundant_uncarved() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let store = Store::default();
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 4.0);
    let cube = catalog::Ground::land as AssetId;
    assert!(!place_val_is_redundant(
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
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let red = catalog::Object::red as AssetId;
    let mut store = Store::default();
    write(&mut store, [x, y, 5.0], GROUND.leaf_cell_zoom() - 1, red);
    write(&mut store, [x, y, 5.0], GROUND.leaf_cell_zoom(), AIR);
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 5.0);
    assert!(!place_val_is_redundant(
        world,
        &at,
        GROUND.leaf_cell_zoom(),
        red
    ));
}

/// Erasing an entry finer than a leaf cell, from a cube placed over the
/// ground, leaves a hole where it was: the ground would fill its cube once
/// it is gone.
#[test]
fn erase_finer_than_a_leaf_leaves_the_hole() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let leaf = GROUND.leaf_cell_zoom();
    let mut store = Store::default();
    write(
        &mut store,
        [x, y, 4.0],
        leaf,
        catalog::Object::red as AssetId,
    );
    write(
        &mut store,
        [x, y, 4.0],
        leaf + 1,
        catalog::Object::blue as AssetId,
    );
    let world = World::new(&tile_cache, &store);
    let entry = Octant::holding(leaf + 1, &world::point(x, y, 4.0));
    assert_eq!(erase_val(world, entry), AIR);
}
