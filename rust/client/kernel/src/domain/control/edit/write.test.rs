// pxl-kernel/domain/control/edit/write.test.rs

use pxl_spec::asset::cube::{self as catalog, Cube};
use pxl_spec::space::coord::world;

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;

/// A whole-cell pick over a carve is redundant only where the ground under the
/// carve is that cube; a cube only ever placed is no ground's.
#[test]
fn place_redundant_only_over_its_own_ground() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    write(&mut store, [x, y, 4.0], GROUND.cell(), AIR);
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 4.0);
    let redundant = |cube| place_val_is_redundant(world, &at, GROUND.cell(), cube);
    assert!(redundant(Cube::ground(catalog::Ground::land)));
    assert!(!redundant(Cube::ground(catalog::Ground::water)));
    assert!(!redundant(Cube::object(catalog::Object::red)));
}

/// Over no carve, a pick is always written.
#[test]
fn place_not_redundant_uncarved() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let store = Store::default();
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 4.0);
    let cube = Cube::ground(catalog::Ground::land);
    assert!(!place_val_is_redundant(world, &at, GROUND.cell(), cube));
}

/// A cube only ever placed is written as asked, even carved out of the same cube.
#[test]
fn place_written_over_a_placed_cube() {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let red = Cube::object(catalog::Object::red);
    let mut store = Store::default();
    write(&mut store, [x, y, 5.0], GROUND.cell() - 1, cube_id(red));
    write(&mut store, [x, y, 5.0], GROUND.cell(), AIR);
    let world = World::new(&tile_cache, &store);
    let at = world::point(x, y, 5.0);
    assert!(!place_val_is_redundant(world, &at, GROUND.cell(), red));
}
