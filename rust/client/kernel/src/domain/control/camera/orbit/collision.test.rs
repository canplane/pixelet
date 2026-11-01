// pxl-kernel/domain/control/camera/orbit/collision.test.rs
//
// The climb, the pose drawn from the pose asked and the world, and the stop
// gate's cells: flat ground at five, and a cube over it for the roof, a
// tunnel's slab, or a block in the eye's way.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::query::over;
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
    let in_solid = |p: &ViewPose| solid(p, ground_under(p, &access));
    assert!(in_solid(&east(1.0)));
    let crossed_to = |at: &ViewPose| crossed(in_solid(&east(1.0)), in_solid(at), at, &access);
    assert!(crossed_to(&east(2.0)));
    assert!(!crossed_to(&east(0.4)));
}

/// The ground at five and a tunnel's roof over the pivot: a slab seven cells
/// across and `thick` cells thick, its underside `roof` over the ground.
fn tunnel(roof: f64, thick: u32) -> (TileCache<TerrainTile>, Store, V3) {
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::Object(cube::Object::red));
    for dz in 0..thick {
        for dx in -3..=3 {
            for dy in -3..=3 {
                let at = [
                    x + f64::from(dx),
                    y + f64::from(dy),
                    5.0 + roof + f64::from(dz),
                ];
                write(&mut store, at, OBJECT_COARSEST, red);
            }
        }
    }
    (tile_cache, store, [x + 0.5, y + 0.5, over(5.0)])
}

/// `at` climbed over the world `(tile_cache, store)`.
fn climb(world: &(TileCache<TerrainTile>, Store, V3), at: ViewPose) -> ViewPose {
    climbed(at, &World::new(&world.0, &world.1))
}

/// Over open ground, with the eye a cell over it, what is drawn is what was
/// asked.
#[test]
fn open_ground_drawn_as_asked() {
    let _g = serial();
    let world = roofed(None);
    let at = pose(world.2, 16.0, 0.8);
    assert_eq!(climb(&world, at), at);
}

/// Level with the pivot the eye rises to a cell over the floor under it,
/// its pitch solved as `acos` of the rise, the distance kept.
#[test]
fn eye_kept_over_floor() {
    let _g = serial();
    let world = roofed(None);
    let at = ViewPose {
        polar: core::f64::consts::FRAC_PI_2,
        ..pose(world.2, 10.0, 0.0)
    };
    let drawn = climb(&world, at);
    let rise = 5.0 + EYE_CLEARANCE_U - world.2[2];
    assert_eq!(drawn.dist, 10.0);
    assert!(
        (drawn.polar - libm::acos(rise / 10.0)).abs() < 1e-9,
        "{drawn:?}"
    );
}

/// A block three cells across and high east of the pivot, in the way of a
/// zoom out: the eye climbs over it at the distance asked, the line from the
/// pivot to it open, as far out as is asked.
#[test]
fn zoom_out_climbs_over_block() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    for dx in [1.5, 2.5, 3.5] {
        for dy in [-1.5, -0.5, 0.5] {
            for z in [5.0, 6.0, 7.0] {
                write(
                    &mut store,
                    [pivot[0] + dx, pivot[1] + dy, z],
                    OBJECT_COARSEST,
                    red,
                );
            }
        }
    }
    let world = (tile_cache, store, pivot);
    let access = World::new(&world.0, &world.1);
    for dist in [6.0, 12.0, 40.0] {
        let at = ViewPose {
            pivot,
            dist,
            polar: 1.3,
            azimuth: -core::f64::consts::FRAC_PI_2,
        };
        assert!(toward_eye(&at, dist, &access).is_some());
        let drawn = climb(&world, at);
        assert_eq!(drawn.dist, dist);
        assert!(drawn.polar < at.polar, "{drawn:?}");
        assert!(toward_eye(&drawn, dist, &access).is_none(), "{drawn:?}");
    }
}

/// A cube straight over the pivot is a roof: no pitch opens the line, and
/// the eye stays on the line asked, `NEAR` short of the cube's underside,
/// however far out is asked; under a roof nearer than the orbit comes, at
/// `MIN_DIST`.
#[test]
fn cube_over_pivot_roofs() {
    let _g = serial();
    for (roof, held) in [(3.0, 8.0 - over(5.0) - NEAR), (2.0, MIN_DIST)] {
        let world = roofed(Some(roof));
        for dist in [10.0, 40.0] {
            let at = pose(world.2, dist, 0.0);
            assert_eq!(
                climb(&world, at),
                ViewPose { dist: held, ..at },
                "roof {roof}, {dist}"
            );
        }
    }
}

/// Under a tunnel's slab, tilted, the pitch asked is kept and the eye stays
/// short of the slab on that line; asked nearer than the slab, as asked.
#[test]
fn tunnel_roofs_on_line_asked() {
    let _g = serial();
    let world = tunnel(3.0, 2);
    let access = World::new(&world.0, &world.1);
    let at = pose(world.2, 16.0, 0.1);
    let roof = toward_eye(&at, 16.0, &access).unwrap();
    assert_eq!(
        climb(&world, at),
        ViewPose {
            dist: roof.t - NEAR,
            ..at
        }
    );
    let near = pose(world.2, 1.5, 0.1);
    assert_eq!(climb(&world, near), near);
}
