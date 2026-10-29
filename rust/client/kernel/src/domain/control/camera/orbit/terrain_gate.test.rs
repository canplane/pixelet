// pxl-kernel/domain/control/camera/orbit/terrain_gate.test.rs
//
// The dive under a roof and the zoom out held under it: flat ground at five,
// and a cube over it for the roof.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;
use pxl_spec::asset::cube::{self, cube_id, Cube};
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::bake::tile::terrain::TerrainTile;

fn pose(pivot: V3, dist: f64, polar: f64) -> ViewPose {
    ViewPose {
        pivot,
        dist,
        polar,
        azimuth: 0.0,
    }
}

/// The ground at five, a roof's underside `roof` over it where there is
/// one, and the pivot standing on the ground under it.
fn roofed(roof: Option<f64>) -> (TileCache<TerrainTile>, Store, V3) {
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    if let Some(roof) = roof {
        let red = cube_id(Cube::Object(cube::Object::red));
        write(&mut store, [x, y, 5.0 + roof], OBJECT_COARSEST, red);
    }
    (tile_cache, store, [x + 0.5, y + 0.5, over(5.0)])
}

/// A zoom from 16 to `to` at the pitch the climb holds: where the eye dives
/// to, if it does.
fn dive_to(roof: Option<f64>, to: f64, polar: f64) -> Option<f64> {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(roof);
    let access = World::new(&tile_cache, &store);
    let to = pose(pivot, to, polar);
    dive(&to, &to, &pose(pivot, 16.0, polar), &access).map(|p| p.dist)
}

/// A zoom out straight down from `from` to `to`: where it is held, if it is.
fn held_at(roof: Option<f64>, from: f64, to: f64) -> Option<f64> {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(roof);
    let access = World::new(&tile_cache, &store);
    held_under(&pose(pivot, to, 0.0), &pose(pivot, from, 0.0), &access).map(|p| p.dist)
}

#[test]
fn dives_under_roof() {
    let under = 8.0 - over(5.0) - NEAR;
    assert_eq!(dive_to(Some(3.0), 10.0, 0.0), Some(under));
    // Asked nearer than the roof, it stands where it was asked.
    assert_eq!(dive_to(Some(3.0), 2.2, 0.0), Some(2.2));
}

#[test]
fn dive_held_back() {
    // A zoom out.
    assert_eq!(dive_to(Some(3.0), 20.0, 0.0), None);
    // Not straight down.
    assert_eq!(dive_to(Some(3.0), 10.0, 0.3), None);
    // Nothing over the pivot.
    assert_eq!(dive_to(None, 10.0, 0.0), None);
    // The orbit's nearest past the roof's lowest cell.
    assert_eq!(dive_to(Some(1.0), 10.0, 0.0), None);
}

/// A roof nearer the pivot than the orbit comes: the eye stands at the
/// orbit's nearest, in the roof's lowest cell.
#[test]
fn dives_into_low_roof() {
    // The roof's underside at 7, its lowest cell to 8; the pivot at 5.5.
    assert_eq!(dive_to(Some(2.0), 10.0, 0.0), Some(MIN_DIST));
}

/// Under the roof, a zoom out goes as far as the roof leaves; from its
/// lowest cell, nowhere; over it, as the climb takes it.
#[test]
fn zoom_out_held_under_roof() {
    let under = 8.0 - over(5.0) - NEAR;
    assert_eq!(held_at(Some(3.0), MIN_DIST, 10.0), Some(under));
    assert_eq!(held_at(Some(2.0), MIN_DIST, 10.0), Some(MIN_DIST));
    assert_eq!(held_at(Some(3.0), 16.0, 20.0), None);
    assert_eq!(held_at(None, MIN_DIST, 10.0), None);
}

/// The roof's lowest cell holds the eye on its floor, not on its ceiling,
/// where the walk takes the eye to be in the cell over it.
#[test]
fn under_half_open() {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(Some(2.0));
    let access = World::new(&tile_cache, &store);
    let at = pose(pivot, MIN_DIST, 0.0);
    let roof = toward_eye(&at, 16.0, &access).unwrap();
    // The underside at 7 and the cell's top at 8, the pivot at 5.5.
    let to = |z: f64| z - pivot[2];
    assert!(under(&roof, &at, to(7.0)));
    assert!(under(&roof, &at, to(7.9)));
    assert!(!under(&roof, &at, to(8.0)));
}

/// The eye may stand in the first cell of a roof or a wall, air under it or
/// beside it, not in the ground's top with air only over it, nor deeper.
#[test]
fn embedded_first_cell() {
    let _g = serial();
    let (tile_cache, mut store, [x, y, _]) = roofed(Some(2.0));
    // A wall on the ground beside the pivot's cell, from 5 to 6.
    let red = cube_id(Cube::Object(cube::Object::red));
    write(&mut store, [x + 0.5, y - 0.5, 5.0], OBJECT_COARSEST, red);
    let access = World::new(&tile_cache, &store);
    // Over the pivot, or a cell beside it, in the frame the gate puts the eye in.
    let at = |dx: f64, z: f64| world_to_canonical(&[x + dx, y, z]);
    // The roof's lowest cell, air under it.
    assert!(embedded(&at(0.0, 7.5), &access));
    // The wall's cell, air beside it.
    assert!(embedded(&at(1.0, 5.5), &access));
    // The ground's top, air only over it.
    assert!(!embedded(&at(0.0, 4.5), &access));
    // Deeper in the ground.
    assert!(!embedded(&at(0.0, 3.5), &access));
    // Air.
    assert!(!embedded(&at(0.0, 6.0), &access));
}

/// Out of a wall's first cell the eye comes back to the pivot's side, not
/// through to the far side, where the wall hides the pivot.
#[test]
fn wall_not_crossed() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    // A wall a cell thick east of the pivot's cell, from 5 to 6.
    let red = cube_id(Cube::Object(cube::Object::red));
    write(
        &mut store,
        [pivot[0] + 0.5, pivot[1] - 0.5, 5.0],
        OBJECT_COARSEST,
        red,
    );
    let access = World::new(&tile_cache, &store);
    // Level with the pivot, `dist` east of it.
    let east = |dist: f64| ViewPose {
        pivot,
        dist,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&east(1.0), &access));
    assert!(crossed(&east(1.0), &east(2.0), &access));
    assert!(!crossed(&east(1.0), &east(0.4), &access));
}
