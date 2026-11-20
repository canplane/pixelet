// pxl-core/domain/control/camera/orbit.test.rs
//
// Tests of the arm under a ceiling and behind a wall, and of the climb.
// The ground is flat at height five. A cube over it makes a ceiling, and a slab
// of cubes makes a tunnel's ceiling.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed, set_eye, withhold_at, write};
use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube::{self};
use pxl_spec::bake::constant::OBJECT_COARSEST;

fn cam_at(pivot: V3, dist: f64, polar: f64) -> Camera {
    Camera {
        pivot,
        dist,
        polar,
        azimuth: 0.0,
    }
}

/// A pose whose eye is level with `pivot` and `dist` east of it.
fn east(pivot: V3, dist: f64) -> Camera {
    Camera {
        pivot,
        dist,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    }
}

/// Writes a red cube at `at`, one cell at the object coarsest zoom.
fn cube_at(store: &mut Store, at: V3) {
    write(store, at, OBJECT_COARSEST, cube::Object::Red as AssetId);
}

/// The ground at five, a ceiling whose underside is `ceiling` above the ground
/// when `ceiling` is given, and the pivot standing on the ground under it.
fn under_ceiling(ceiling: Option<f64>) -> (Store, V3) {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    if let Some(ceiling) = ceiling {
        cube_at(&mut store, [x, y, 5.0 + ceiling]);
    }
    (store, [x + 0.5, y + 0.5, over(5.0)])
}

/// Over open ground, beyond the nearest, the eye keeps one cell over its
/// floor. At the nearest it stands as asked.
#[test]
fn open_ground_keeps_a_cell() {
    let (store, pivot) = under_ceiling(None);
    let world = placed(store);
    let level = core::f64::consts::FRAC_PI_2;
    assert_eq!(
        at_climb(cam_at(pivot, MIN_DIST, level), &world).polar,
        level
    );
    // At the nearest below the pivot's level, a tilt in the air is drawn as
    // asked, and a tilt into the floor is drawn at the floor.
    let low = at_climb(cam_at(pivot, MIN_DIST, 1.7), &world);
    assert_eq!(low.polar, 1.7);
    let floored = at_climb(cam_at(pivot, MIN_DIST, 2.2), &world);
    assert!(floored.polar > level && floored.polar < 2.2, "{floored:?}");
    assert!(!in_solid(&floored, &world), "{floored:?}");
    for dist in [NEAREST_DIST + 0.01, 3.0, 16.0] {
        let at = cam_at(pivot, dist, core::f64::consts::FRAC_PI_2);
        let climbed = at_climb(at, &world).polar;
        assert!(
            (climbed - libm::acos(0.5 / dist)).abs() < 1e-9,
            "{dist} {climbed}"
        );
    }
}

/// At the orbit's nearest, the eye turns above the pivot's level whatever is
/// in the way. Turned into a wall level with the pivot and a cell beside it,
/// or into a thick block round the pivot, the climb keeps the camera's own pitch.
/// Beyond the nearest, and below the pivot's level, the eye climbs and stops
/// as it does anywhere.
#[test]
fn nearest_turns_through() {
    let (mut store, pivot) = under_ceiling(None);
    for dx in [-2.5, -1.5, 1.5, 2.5] {
        for dy in [-2.5, -1.5, -0.5, 0.5, 1.5] {
            for z in [5.0, 6.0, 7.0] {
                cube_at(&mut store, [pivot[0] + dx, pivot[1] + dy, z]);
            }
        }
    }
    let world = placed(store);
    for polar in [0.4, 1.0, core::f64::consts::FRAC_PI_2] {
        for azimuth in [-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2] {
            let at = Camera {
                polar,
                azimuth,
                ..east(pivot, MIN_DIST)
            };
            let climbed = at_climb(at, &world);
            assert_eq!(climbed, at);
        }
    }
    let at = east(pivot, 3.0);
    assert!(in_solid(&at, &world));
    assert!(at_climb(at, &world).polar < at.polar);
    // The nearest counts out to `NEAREST_DIST`, and no farther.
    let edge = Camera {
        dist: NEAREST_DIST,
        ..at
    };
    assert!(in_solid(&edge, &world));
    assert_eq!(at_climb(edge, &world).polar, at.polar);
    let past_edge = Camera {
        dist: NEAREST_DIST + 0.01,
        ..at
    };
    assert!(at_climb(past_edge, &world).polar < at.polar);
}

/// The ground at five and a tunnel's ceiling over the pivot. The ceiling is a slab
/// seven cells across and `thick` cells thick, with its underside `ceiling`
/// above the ground.
fn tunnel(ceiling: f64, thick: u32) -> (Store, V3) {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    for dz in 0..thick {
        for dx in -3..=3 {
            for dy in -3..=3 {
                let at = [
                    x + f64::from(dx),
                    y + f64::from(dy),
                    5.0 + ceiling + f64::from(dz),
                ];
                cube_at(&mut store, at);
            }
        }
    }
    (store, [x + 0.5, y + 0.5, over(5.0)])
}

/// In the ceiling's lowest cell, where the eye may stand, the climb asks for no
/// rise. A tilt there is drawn as asked and is not held straight down.
#[test]
fn climb_stands_in_ceiling_cell() {
    let (store, pivot) = tunnel(2.0, 1);
    let world = placed(store);
    let climbed = at_climb(cam_at(pivot, MIN_DIST, 0.3), &world);
    assert_eq!(climbed.polar, 0.3);
}

/// In a ceiling thicker than its lowest cell, even straight down does not
/// clear it, so the climb keeps the camera's own pitch, on the way in and once there.
#[test]
fn climb_fails_under_ceiling() {
    let (store, pivot) = tunnel(3.0, 2);
    let world = placed(store);
    // The eye is in the slab's upper cell, between heights 9 and 10.
    let climbed = at_climb(cam_at(pivot, 3.8, 0.1), &world);
    assert_eq!(climbed.polar, 0.1);
}

/// Over open ground the climb steepens. A block three cells across and
/// three high round the eye, deeper than one cell, lifts the eye to clear the
/// block's top.
#[test]
fn climb_clears_block() {
    let (mut store, pivot) = under_ceiling(None);
    // The block surrounds the cell where the eye stands, level with the pivot
    // and 3 east of it.
    for dx in [1.5, 2.5, 3.5] {
        for dy in [-1.5, -0.5, 0.5] {
            for z in [5.0, 6.0, 7.0] {
                cube_at(&mut store, [pivot[0] + dx, pivot[1] + dy, z]);
            }
        }
    }
    let world = placed(store);
    let at = east(pivot, 3.0);
    let climbed = at_climb(at, &world);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &world), "{climbed:?}");
}

/// Beyond the nearest, the eye does not stand in a wall. Asked level with
/// the pivot into a wall one cell thick and three cells east, the eye climbs
/// over the wall instead of into it.
#[test]
fn climbs_out_of_wall_with_room() {
    let (mut store, pivot) = under_ceiling(None);
    for dy in [-1.5, -0.5, 0.5, 1.5] {
        cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + dy, 5.0]);
    }
    let world = placed(store);
    let at = east(pivot, 3.0);
    assert!(in_solid(&at, &world));
    let climbed = at_climb(at, &world);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &world), "{climbed:?}");
}

/// The rise is read under the eye itself, however far the eye is. Under a
/// slab, an eye in a wall that meets the slab rises by the floor under it.
#[test]
fn climb_rises_under_eye() {
    let (mut store, pivot) = tunnel(3.0, 1);
    for dy in [-1.5, -0.5, 0.5, 1.5] {
        for z in [5.0, 6.0, 7.0] {
            cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + dy, z]);
        }
    }
    let world = placed(store);
    let at = east(pivot, 3.0);
    assert!(in_solid(&at, &world));
    let rise = climb_rise(&at, &world).expect("a rise under the eye");
    assert!(rise > 0.0, "{rise}");
}

/// Ground that two passes do not climb out of is not a ceiling, so the climbed
/// pitch is kept, not the asked one. The eye is level with the pivot, 40
/// out, in ground that rises a cell for each cell toward the pivot. Each
/// pass steepens the pitch and draws the eye in over higher ground, so two
/// passes do not clear it, yet straight down does.
#[test]
fn slope_keeps_pitch_climbed() {
    let (mut store, pivot) = under_ceiling(None);
    // From 20 to 40 cells east, the fill over each cell is 60 cells high less its
    // distance.
    for dx in 20..=40 {
        for z in 0..60 - dx {
            let at = [pivot[0] + f64::from(dx), pivot[1] - 0.5, 5.0 + f64::from(z)];
            cube_at(&mut store, at);
        }
    }
    let world = placed(store);
    let at = east(pivot, 40.0);
    assert!(in_solid(&at, &world));
    let climbed = at_climb(at, &world);
    assert!(climbed.polar < at.polar, "{climbed:?}");
}

/// The pivot stands on a tower ten cells high. The eye is tilted below the
/// level, 18 out, and sunk in the ground there, clear of the seed's caves.
/// The floor under the eye lies below the pivot, so the climb lifts the eye
/// onto that floor and keeps it below the level.
#[test]
fn climbs_past_level() {
    let (mut store, ground) = under_ceiling(None);
    for z in 0..10 {
        cube_at(&mut store, [ground[0], ground[1], 5.0 + f64::from(z)]);
    }
    let world = placed(store);
    let at = cam_at([ground[0], ground[1], over(15.0)], 18.0, 2.2);
    assert!(in_solid(&at, &world));
    let climbed = at_climb(at, &world);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(climbed.polar > core::f64::consts::FRAC_PI_2, "{climbed:?}");
    assert!(!in_solid(&climbed, &world), "{climbed:?}");
}

/// The arm ends the near plane's distance short of the first thing on the
/// line from the pivot toward the eye: under a ceiling, and before a wall
/// level with the pivot. Where the line is open, or passes beside what is
/// there, it is the distance asked. It is never shorter than the orbit's
/// nearest, and what a coarser tile stands in for is met too.
#[test]
fn the_arm_ends_short_of_what_is_on_its_line() {
    let arm_at = |at: &Camera, world: &World| arm_of(at, OrbitObstacleMode::Stop, world);
    let (store, pivot) = under_ceiling(Some(3.0));
    let world = placed(store);
    let under = 8.0 - over(5.0) - NEAR;
    assert_eq!(arm_at(&cam_at(pivot, 10.0, 0.0), &world), under);
    assert_eq!(
        arm_at(&cam_at(pivot, 2.2, 0.0), &world),
        2.2,
        "nearer than it"
    );
    assert_eq!(arm_at(&cam_at(pivot, 10.0, 0.3), &world), 10.0, "beside it");
    let slanted = arm_at(&cam_at(pivot, 10.0, 0.1), &world);
    assert!((slanted - ((8.0 - over(5.0)) / libm::cos(0.1) - NEAR)).abs() < 1e-6);
    withhold_at([pivot[0], pivot[1]]);
    assert_eq!(
        arm_at(&cam_at(pivot, 10.0, 0.0), &world),
        under,
        "stood in for"
    );

    let (store, pivot) = under_ceiling(Some(1.0));
    let world = placed(store);
    assert_eq!(arm_at(&cam_at(pivot, 10.0, 0.0), &world), MIN_DIST);

    let (mut store, pivot) = under_ceiling(None);
    for dy in -3..=3 {
        cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + f64::from(dy), 5.0]);
    }
    let world = placed(store);
    assert_eq!(
        arm_at(&east(pivot, 6.0), &world),
        2.5 - NEAR,
        "before a wall"
    );
    assert_eq!(arm_at(&east(pivot, 2.3), &world), 2.3);
    assert_eq!(
        arm_at(&cam_at(pivot, 6.0, 0.0), &world),
        6.0,
        "open over it"
    );
    // Under `climb` the arm reaches only where the eye is in something
    // solid, and under `through` never.
    let behind = east(pivot, 6.0);
    assert!(!in_solid(&behind, &world));
    assert_eq!(arm_of(&behind, OrbitObstacleMode::Climb, &world), 6.0);
    let (store, pivot) = tunnel(3.0, 2);
    let world = placed(store);
    let inside = cam_at(pivot, 3.8, 0.0);
    assert!(in_solid(&inside, &world));
    let under = 8.0 - over(5.0) - NEAR;
    assert_eq!(arm_of(&inside, OrbitObstacleMode::Climb, &world), under);
    assert_eq!(arm_of(&inside, OrbitObstacleMode::Through, &world), 3.8);
}

/// The climb holds the pitch the orbit is drawn at: it eases there and
/// rests, a tilt by hand under what it holds changes nothing drawn, and a
/// tilt above it is drawn at once.
#[test]
fn the_climb_holds_the_drawn_pitch() {
    let (store, pivot) = under_ceiling(None);
    placed(store);
    level();
    let level_polar = core::f64::consts::FRAC_PI_2;
    let mut cam = cam_at(pivot, 16.0, level_polar);
    set_eye(eye(&cam));
    let clear = libm::acos(0.5 / 16.0);
    assert_eq!(drawn(&cam).polar, level_polar, "nothing held yet");
    let rested = (0..600).find(|_| !climb_step(&cam, 1.0 / 60.0));
    assert!(rested.is_some(), "the climb rests");
    assert!(
        (drawn(&cam).polar - clear).abs() < 1e-9,
        "at the clearing pitch"
    );
    cam.polar = 1.7;
    assert!((drawn(&cam).polar - clear).abs() < 1e-9, "a tilt under it");
    cam.polar = 1.2;
    assert_eq!(drawn(&cam).polar, 1.2, "a tilt above it, at once");
    level();
}
