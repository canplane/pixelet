// pxl-kernel/domain/control/camera/orbit/collision.test.rs
//
// The dive under a roof and the zoom out held under it, and the climb beside
// them: flat ground at five, and a cube over it for the roof, or a tunnel's
// slab.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::layer::tile_cache::TileCache;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, write};
use crate::sys::serial;
use pxl_content::cave::hollow;
use pxl_engine::answer::FULL;
use pxl_engine::LEAF_CELL;
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

/// A zoom straight down from `from` to `to` in `climb`: where it gets.
fn climbed_to(roof: Option<f64>, from: f64, to: f64) -> f64 {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(roof);
    let access = World::new(&tile_cache, &store);
    climb_step(&pose(pivot, to, 0.0), &pose(pivot, from, 0.0), &access).dist
}

#[test]
fn dives_under_roof() {
    let under = 8.0 - over(5.0) - NEAR;
    assert_eq!(dive_to(Some(3.0), 10.0, 0.0), Some(under));
    // Asked nearer than the roof, it stands where it was asked.
    assert_eq!(dive_to(Some(3.0), 2.2, 0.0), Some(2.2));
}

/// Short of straight down, wherever the climb stopped, it dives as well,
/// on its own line.
#[test]
fn dives_slanted() {
    let polar: f64 = 0.1;
    let under = (8.0 - over(5.0)) / libm::cos(polar) - NEAR;
    let dist = dive_to(Some(3.0), 10.0, polar).unwrap();
    assert!((dist - under).abs() < 1e-6);
}

#[test]
fn dive_held_back() {
    // A zoom out.
    assert_eq!(dive_to(Some(3.0), 20.0, 0.0), None);
    // A line that passes the roof by.
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

/// A zoom out is never held: under a roof, from its lowest cell, over it or
/// with nothing over the pivot, it gets where it is asked; a zoom in into a
/// roof the climb fails in goes under it.
#[test]
fn zoom_out_not_held() {
    assert_eq!(climbed_to(Some(3.0), MIN_DIST, 10.0), 10.0);
    assert_eq!(climbed_to(Some(2.0), MIN_DIST, 10.0), 10.0);
    assert_eq!(climbed_to(Some(3.0), 16.0, 20.0), 20.0);
    assert_eq!(climbed_to(None, MIN_DIST, 10.0), 10.0);
    // Into a slab two cells thick, which the climb fails in: under it.
    let _g = serial();
    let (tile_cache, store, pivot) = tunnel(3.0, 2);
    let access = World::new(&tile_cache, &store);
    let to = climb_step(&pose(pivot, 3.8, 0.0), &pose(pivot, 16.0, 0.0), &access);
    assert_eq!(to.dist, 8.0 - over(5.0) - NEAR);
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
    let crossed_to = |at: &ViewPose| {
        let solid = |p: &ViewPose| in_solid(p, &access);
        crossed(solid(&east(1.0)), solid(at), at, &access)
    };
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

/// In the roof's lowest cell, where the eye may stand, the climb asks no
/// rise: a tilt there is drawn as asked, not held straight down.
#[test]
fn climb_stands_in_roof_cell() {
    let _g = serial();
    let (tile_cache, store, pivot) = tunnel(2.0, 1);
    let access = World::new(&tile_cache, &store);
    let (climbed, failed) = climb_pose(pose(pivot, MIN_DIST, 0.3), &access);
    assert_eq!((climbed.polar, failed), (0.3, false));
}

/// In a roof thicker than its lowest cell, straight down does not clear: the
/// climb fails and keeps the pitch asked, on the way in as at rest.
#[test]
fn climb_fails_under_roof() {
    let _g = serial();
    let (tile_cache, store, pivot) = tunnel(3.0, 2);
    let access = World::new(&tile_cache, &store);
    // The eye in the slab's upper cell, from 9 to 10.
    let (climbed, failed) = climb_pose(pose(pivot, 3.8, 0.1), &access);
    assert_eq!((climbed.polar, failed), (0.1, true));
}

/// Over open ground the climb steepens: a block three cells across and high
/// round the eye, deeper than a first cell, lifts it to clear the block's
/// top.
#[test]
fn climb_clears_block() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    // About the cell east of the pivot where the eye stands level with it 3 off.
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
    let access = World::new(&tile_cache, &store);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    let (climbed, failed) = climb_pose(at, &access);
    assert!(!failed);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}

/// With room at the orbit's nearest, the eye does not stand in a wall's
/// first cell farther out: level with the pivot, the eye asked into a wall
/// a cell thick three cells east climbs over it, not into it.
#[test]
fn climbs_out_of_wall_with_room() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    for dy in [-1.5, -0.5, 0.5, 1.5] {
        write(
            &mut store,
            [pivot[0] + 3.0, pivot[1] + dy, 5.0],
            OBJECT_COARSEST,
            red,
        );
    }
    let access = World::new(&tile_cache, &store);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    assert!(embedded(&eye(&at).1, &access));
    assert!(!cramped(&at, &access));
    let (climbed, failed) = climb_pose(at, &access);
    assert!(!failed);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}

/// The rise is read under the eye itself, however far it is: under a slab,
/// the eye in a wall that meets the slab rises by the floor under it.
#[test]
fn climb_rises_under_eye() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = tunnel(3.0, 1);
    let red = cube_id(Cube::Object(cube::Object::red));
    for dy in [-1.5, -0.5, 0.5, 1.5] {
        for z in [5.0, 6.0, 7.0] {
            write(
                &mut store,
                [pivot[0] + 3.0, pivot[1] + dy, z],
                OBJECT_COARSEST,
                red,
            );
        }
    }
    let access = World::new(&tile_cache, &store);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    let rise = climb_rise(&at, &access, false).expect("a rise under the eye");
    assert!(rise > 0.0, "{rise}");
}

/// Ground that two passes do not climb out of is no roof: the pitch climbed
/// is kept, not the one asked. Level with the pivot, 40 out, the eye in
/// ground that rises a cell for each cell toward the pivot: each pass's
/// steepening draws the eye in over higher ground, and two do not clear it,
/// yet straight down does.
#[test]
fn slope_keeps_pitch_climbed() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    // From 20 cells east to 40, a column 60 less its distance high.
    for dx in 20..=40 {
        for z in 0..60 - dx {
            write(
                &mut store,
                [pivot[0] + f64::from(dx), pivot[1] - 0.5, 5.0 + f64::from(z)],
                OBJECT_COARSEST,
                red,
            );
        }
    }
    let access = World::new(&tile_cache, &store);
    let at = ViewPose {
        pivot,
        dist: 40.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    let (climbed, failed) = climb_pose(at, &access);
    assert!(!failed, "{climbed:?}");
    assert!(climbed.polar < at.polar, "{climbed:?}");
}

/// The pivot on a tower ten over the ground, the eye tilted past level and
/// out to where it lies under the ground in one of the seed's caves: in a
/// cave's air it is not in the ground, and nothing climbs it.
#[test]
fn a_cave_is_not_climbed_out_of() {
    let _g = serial();
    let (tile_cache, mut store, ground) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    for z in 0..10 {
        write(
            &mut store,
            [ground[0], ground[1], 5.0 + f64::from(z)],
            OBJECT_COARSEST,
            red,
        );
    }
    let access = World::new(&tile_cache, &store);
    let pivot = [ground[0], ground[1], over(15.0)];
    // A pose whose eye is in a leaf the seed's caves take under the ground.
    let at = (0..90)
        .flat_map(|a| (0..50).flat_map(move |d| (0..20).map(move |p| (a, d, p))))
        .map(|(a, d, p)| ViewPose {
            azimuth: core::f64::consts::TAU * f64::from(a) / 90.0,
            ..pose(pivot, 10.0 + f64::from(d), 1.9 + 0.05 * f64::from(p))
        })
        .find(|at| {
            let e = eye(at).0;
            e[2] < 4.0 && hollow((LEAF_CELL, e.map(|v| v.floor() as i32))) >= FULL
        })
        .expect("an eye in a cave under the ground");
    assert!(!in_solid(&at, &access), "{at:?}");
    let (climbed, failed) = climb_pose(at, &access);
    assert!(!failed, "{climbed:?}");
    assert_eq!(climbed.polar, at.polar, "{climbed:?}");
}

/// The pivot on a tower ten over the ground, the eye tilted past level 18
/// out and sunk in the ground there, clear of the seed's caves: the floor under the eye lies under the
/// pivot, and the climb lifts it past level onto that floor.
#[test]
fn climbs_past_level() {
    let _g = serial();
    let (tile_cache, mut store, ground) = roofed(None);
    let red = cube_id(Cube::Object(cube::Object::red));
    for z in 0..10 {
        write(
            &mut store,
            [ground[0], ground[1], 5.0 + f64::from(z)],
            OBJECT_COARSEST,
            red,
        );
    }
    let access = World::new(&tile_cache, &store);
    let at = pose([ground[0], ground[1], over(15.0)], 18.0, 2.2);
    assert!(in_solid(&at, &access));
    let (climbed, failed) = climb_pose(at, &access);
    assert!(!failed, "{climbed:?}");
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(climbed.polar > core::f64::consts::FRAC_PI_2, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}
