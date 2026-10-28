// pxl-kernel/domain/control/camera/orbit/terrain_gate.test.rs
//
// The dive under a roof: flat ground at five, and a cube over it for the roof.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;
use pxl_spec::asset::cube::{self, cube_id, Cube};
use pxl_spec::bake::constant::OBJECT_COARSEST;

fn pose(target: V3, dist: f64, polar: f64) -> ViewPose {
    ViewPose {
        target,
        dist,
        polar,
        azimuth: 0.0,
    }
}

/// A zoom from 16 to `to` at the pitch the climb holds, over a focus standing
/// on the ground with a roof's underside `roof` over the ground: where the
/// eye dives to, if it does.
fn dive_to(roof: Option<f64>, to: f64, polar: f64) -> Option<f64> {
    let _g = serial();
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    if let Some(roof) = roof {
        let red = cube_id(Cube::Object(cube::Object::red));
        write(&mut store, [x, y, 5.0 + roof], OBJECT_COARSEST, red);
    }
    let access = World::new(&tile_cache, &store);
    let target = [x + 0.5, y + 0.5, over(5.0)];
    let to = pose(target, to, polar);
    dive(&to, &to, &pose(target, 16.0, polar), &access).map(|p| p.dist)
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
    // Nothing over the focus.
    assert_eq!(dive_to(None, 10.0, 0.0), None);
    // The orbit's nearest past the roof's lowest cell.
    assert_eq!(dive_to(Some(1.0), 10.0, 0.0), None);
}

/// A roof nearer the focus than the orbit comes: the eye stands at the
/// orbit's nearest, in the roof's lowest cell.
#[test]
fn dives_into_low_roof() {
    // The roof's underside at 7, its lowest cell to 8; the focus at 5.5.
    assert_eq!(dive_to(Some(2.0), 10.0, 0.0), Some(MIN_DIST));
}
