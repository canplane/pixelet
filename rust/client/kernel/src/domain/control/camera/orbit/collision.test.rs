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
        let red = cube_id(Cube::object(cube::Object::red));
        write(&mut store, [x, y, 5.0 + roof], OBJECT_COARSEST, red);
    }
    (tile_cache, store, [x + 0.5, y + 0.5, over(5.0)])
}

/// A zoom from 16 to `to` at the pitch the climb holds: where the eye dives
/// to, if it does.
fn dive_to(roof: Option<f64>, to: f64, polar: f64) -> Option<f64> {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(roof);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let to = pose(pivot, to, polar);
    dive(&to, &pose(pivot, 16.0, polar), &access).map(|p| p.dist)
}

/// A zoom straight down from `from` to `to` in `climb`: where it gets.
fn climbed_to(roof: Option<f64>, from: f64, to: f64) -> f64 {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(roof);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
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
    // A line that passes the roof by.
    assert_eq!(dive_to(Some(3.0), 10.0, 0.3), None);
    // Nothing over the pivot.
    assert_eq!(dive_to(None, 10.0, 0.0), None);
}

/// A roof nearer the pivot than the orbit comes: the zoom goes to the orbit's
/// nearest.
#[test]
fn dives_into_low_roof() {
    assert_eq!(dive_to(Some(2.0), 10.0, 0.0), Some(MIN_DIST));
    assert_eq!(dive_to(Some(1.0), 10.0, 0.0), Some(MIN_DIST));
}

/// A zoom out goes as far as it gets: under a roof, up to it, at the nearest
/// in its lowest cell, not out; from over the roof, or with nothing over the
/// pivot, where it is asked. A zoom in whose way hit a slab goes under it, from
/// over it or from in it; a zoom out from in it stops.
#[test]
fn zoom_out_does_not_go_through() {
    let held = climbed_to(Some(3.0), MIN_DIST, 10.0);
    assert!((MIN_DIST..=8.0 - over(5.0)).contains(&held), "{held}");
    assert_eq!(climbed_to(Some(2.0), MIN_DIST, 10.0), MIN_DIST);
    assert_eq!(climbed_to(Some(3.0), 16.0, 20.0), 20.0);
    assert_eq!(climbed_to(None, MIN_DIST, 10.0), 10.0);
    let _g = serial();
    let (tile_cache, store, pivot) = tunnel(3.0, 2);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let to = climb_step(&pose(pivot, 3.8, 0.0), &pose(pivot, 16.0, 0.0), &access);
    assert_eq!(to.dist, 8.0 - over(5.0) - NEAR);
    // From in the slab: a zoom out stops where it stands, a zoom in dives.
    let out = climb_step(&pose(pivot, 4.2, 0.0), &pose(pivot, 3.8, 0.0), &access);
    assert_eq!(out.dist, 3.8);
    let down = climb_step(&pose(pivot, 3.4, 0.0), &pose(pivot, 3.8, 0.0), &access);
    assert_eq!(down.dist, 8.0 - over(5.0) - NEAR);
}

/// A low wall on the way climbs over it, a zoom in and a zoom out alike, the
/// distance granted, from level as from over it. Under a slab that meets it,
/// the climb hits the slab on its way up: a zoom out stops short, a zoom in
/// dives through the wall.
#[test]
fn climb_or_dive_or_stop() {
    let _g = serial();
    let wall = |store: &mut Store, pivot: V3, top: f64| {
        let red = cube_id(Cube::object(cube::Object::red));
        for dy in -3..=3 {
            for z in [5.0, 6.0].into_iter().filter(|&z| z < top) {
                write(
                    store,
                    [pivot[0] + 3.0, pivot[1] + f64::from(dy), z],
                    OBJECT_COARSEST,
                    red,
                );
            }
        }
    };
    let east = |pivot: V3, dist: f64| ViewPose {
        pivot,
        dist,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };

    let (tile_cache, mut store, pivot) = roofed(None);
    wall(&mut store, pivot, 6.0);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let to = east(pivot, 3.2);
    assert!(climb_pose(to, &access).polar < to.polar);
    assert_eq!(climb_step(&to, &east(pivot, 2.4), &access), to);
    assert_eq!(climb_step(&to, &east(pivot, 4.2), &access), to);
    let tilted = |dist: f64| ViewPose {
        polar: 1.2,
        ..east(pivot, dist)
    };
    assert_eq!(climb_step(&tilted(3.2), &tilted(2.6), &access), tilted(3.2));

    let (tile_cache, mut store, pivot) = tunnel(2.0, 1);
    wall(&mut store, pivot, 7.0);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let to = east(pivot, 3.2);
    let stopped = climb_step(&to, &east(pivot, 2.4), &access).dist;
    assert!((2.4..3.2).contains(&stopped), "{stopped}");
    // Over a slab it clears the corner of, it climbs; in one too thick, a zoom in
    // dives through the wall.
    assert_eq!(climb_step(&to, &east(pivot, 4.2), &access), to);
    let (tile_cache, mut store, pivot) = tunnel(2.0, 2);
    wall(&mut store, pivot, 7.0);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let to = east(pivot, 3.2);
    let dived = climb_step(&to, &east(pivot, 4.2), &access).dist;
    assert!((dived - (2.5 - NEAR)).abs() < 1e-9, "{dived}");
}

/// Over open ground the eye keeps a cell over its floor past the nearest; at
/// the nearest it stands as asked.
#[test]
fn open_ground_keeps_a_cell() {
    let _g = serial();
    let (tile_cache, store, pivot) = roofed(None);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let level = core::f64::consts::FRAC_PI_2;
    assert_eq!(
        climb_pose(pose(pivot, MIN_DIST, level), &access).polar,
        level
    );
    // Under the level at the nearest: as asked in the air; a tilt into the floor
    // stops short of it, from any pitch, and the eye stays out of it.
    let low = 1.7;
    let down = climb_step(
        &pose(pivot, MIN_DIST, low),
        &pose(pivot, MIN_DIST, 1.6),
        &access,
    );
    assert_eq!(down.polar, low);
    let floored = climb_step(
        &pose(pivot, MIN_DIST, 2.2),
        &pose(pivot, MIN_DIST, 1.7),
        &access,
    );
    assert!(floored.polar < 2.2, "{floored:?}");
    assert!(!in_solid(&floored, &access), "{floored:?}");
    for dist in [NEAREST_DIST + 0.01, 3.0, 16.0] {
        let at = pose(pivot, dist, core::f64::consts::FRAC_PI_2);
        let climbed = climb_pose(at, &access).polar;
        assert!(
            (climbed - libm::acos(0.5 / dist)).abs() < 1e-9,
            "{dist} {climbed}"
        );
    }
}

/// At the orbit's nearest the eye turns over the pivot's level whatever is
/// in the way: into a wall level with the pivot, a cell beside it, and into a
/// thick block round the pivot, the climb keeps the pitch asked; under the
/// pivot's level it climbs as anywhere.
#[test]
fn nearest_turns_through() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for dx in [-2.5, -1.5, 1.5, 2.5] {
        for dy in [-2.5, -1.5, -0.5, 0.5, 1.5] {
            for z in [5.0, 6.0, 7.0] {
                let at = [pivot[0] + dx, pivot[1] + dy, z];
                write(&mut store, at, OBJECT_COARSEST, red);
            }
        }
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    for polar in [0.4, 1.0, core::f64::consts::FRAC_PI_2] {
        for azimuth in [-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2] {
            let at = ViewPose {
                pivot,
                dist: MIN_DIST,
                polar,
                azimuth,
            };
            let climbed = climb_pose(at, &access);
            assert_eq!(climbed, at);
        }
    }
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    assert!(climb_pose(at, &access).polar < at.polar);
    // The nearest counts out to `NEAREST_DIST`, and no farther.
    let edge = ViewPose {
        dist: NEAREST_DIST,
        ..at
    };
    assert!(in_solid(&edge, &access));
    assert_eq!(climb_pose(edge, &access).polar, at.polar);
    let past_edge = ViewPose {
        dist: NEAREST_DIST + 0.01,
        ..at
    };
    assert!(climb_pose(past_edge, &access).polar < at.polar);
    // Asked past the level in a wall, it stays where it stands.
    let level = ViewPose {
        dist: MIN_DIST,
        ..at
    };
    let past = ViewPose {
        polar: 2.2,
        ..level
    };
    assert_eq!(climb_step(&past, &level, &access), level);
}

/// `stop` goes the straight line alone: into a low wall it stops short, from
/// either side, a zoom in as a zoom out.
#[test]
fn stop_goes_straight() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for dy in -3..=3 {
        let at = [pivot[0] + 3.0, pivot[1] + f64::from(dy), 5.0];
        write(&mut store, at, OBJECT_COARSEST, red);
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let east = |dist: f64| ViewPose {
        pivot,
        dist,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    let dist_at = |from: f64, to: f64| {
        way_hit(&east(from), &east(to), |at| *at, &access).map(|t| from + (to - from) * t)
    };
    assert!(dist_at(2.3, 6.0).unwrap() < 3.0);
    assert!(dist_at(6.0, 2.8).unwrap() > 3.0);
    assert_eq!(dist_at(2.3, 2.45), None);
}

/// The ground at five and a tunnel's roof over the pivot: a slab seven cells
/// across and `thick` cells thick, its underside `roof` over the ground.
fn tunnel(roof: f64, thick: u32) -> (TileCache<TerrainTile>, Store, V3) {
    let (tile_cache, [x, y]) = flat_at_five();
    let mut store = Store::default();
    let red = cube_id(Cube::object(cube::Object::red));
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
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let climbed = climb_pose(pose(pivot, MIN_DIST, 0.3), &access);
    assert_eq!(climbed.polar, 0.3);
}

/// In a roof thicker than its lowest cell, straight down does not clear: the
/// climb keeps the pitch asked, on the way in as at rest.
#[test]
fn climb_fails_under_roof() {
    let _g = serial();
    let (tile_cache, store, pivot) = tunnel(3.0, 2);
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    // The eye in the slab's upper cell, from 9 to 10.
    let climbed = climb_pose(pose(pivot, 3.8, 0.1), &access);
    assert_eq!(climbed.polar, 0.1);
}

/// Over open ground the climb steepens: a block three cells across and high
/// round the eye, deeper than a first cell, lifts it to clear the block's
/// top.
#[test]
fn climb_clears_block() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
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
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    let climbed = climb_pose(at, &access);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}

/// Farther out than the nearest, the eye does not stand in a wall: level with the pivot, the eye asked into a wall
/// a cell thick three cells east climbs over it, not into it.
#[test]
fn climbs_out_of_wall_with_room() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for dy in [-1.5, -0.5, 0.5, 1.5] {
        write(
            &mut store,
            [pivot[0] + 3.0, pivot[1] + dy, 5.0],
            OBJECT_COARSEST,
            red,
        );
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    let climbed = climb_pose(at, &access);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}

/// The rise is read under the eye itself, however far it is: under a slab,
/// the eye in a wall that meets the slab rises by the floor under it.
#[test]
fn climb_rises_under_eye() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = tunnel(3.0, 1);
    let red = cube_id(Cube::object(cube::Object::red));
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
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let at = ViewPose {
        pivot,
        dist: 3.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    let rise = climb_rise(&at, &access).expect("a rise under the eye");
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
    let red = cube_id(Cube::object(cube::Object::red));
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
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let at = ViewPose {
        pivot,
        dist: 40.0,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    assert!(in_solid(&at, &access));
    let climbed = climb_pose(at, &access);
    assert!(climbed.polar < at.polar, "{climbed:?}");
}

/// The pivot on a tower ten over the ground and under a roof, the eye tilted
/// past level and out to where it lies under the ground in one of the seed's
/// caves: with the pivot roofed, a cave's air is not in the ground, and nothing
/// climbs it.
#[test]
fn a_cave_is_not_climbed_out_of() {
    let _g = serial();
    let (tile_cache, mut store, ground) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for z in (0..10).chain([12]) {
        write(
            &mut store,
            [ground[0], ground[1], 5.0 + f64::from(z)],
            OBJECT_COARSEST,
            red,
        );
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
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
    let climbed = climb_pose(at, &access);
    assert_eq!(climbed.polar, at.polar, "{climbed:?}");
}

/// With the sky open over the pivot, a hollow in a hill is no place for the
/// eye: a zoom out and a zoom in over the hill stay over it, the distance
/// granted. With the pivot in the hollow, the eye stays in it as asked.
#[test]
fn a_hollow_in_a_hill() {
    let _g = serial();
    let (tile_cache, mut store, pivot) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for dx in 8..=40 {
        for dy in -6..=6 {
            for z in (0..12).filter(|z| !((18..=30).contains(&dx) && (3..=7).contains(z))) {
                let at = [
                    pivot[0] + f64::from(dx),
                    pivot[1] + f64::from(dy),
                    5.0 + f64::from(z),
                ];
                write(&mut store, at, OBJECT_COARSEST, red);
            }
        }
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let east = |pivot: V3, dist: f64| ViewPose {
        pivot,
        dist,
        polar: 1.45,
        azimuth: -core::f64::consts::FRAC_PI_2,
    };
    let over_hill = |dist: f64| {
        let e = eye(&climb_pose(east(pivot, dist), &access)).0;
        let x = e[0] - pivot[0];
        assert!(!(8.5..40.0).contains(&x) || e[2] >= 17.0, "{dist} {e:?}");
    };
    let mut d: f64 = 6.0;
    while d < 60.0 {
        let to = east(pivot, d * 1.1);
        assert_eq!(climb_step(&to, &east(pivot, d), &access), to);
        over_hill(to.dist);
        d = to.dist;
    }
    while d > 6.0 {
        let to = east(pivot, d / 1.1);
        assert_eq!(climb_step(&to, &east(pivot, d), &access), to);
        over_hill(to.dist);
        d = to.dist;
    }
    let inside = [pivot[0] + 22.0, pivot[1], over(8.0)];
    let at = east(inside, 4.0);
    assert!(!in_solid(&at, &access));
    assert!(eye(&climb_pose(at, &access)).0[2] < 13.0);
}

/// The pivot on a tower ten over the ground, the eye tilted past level 18
/// out and sunk in the ground there, clear of the seed's caves: the floor under the eye lies under the
/// pivot, and the climb lifts it past level onto that floor.
#[test]
fn climbs_past_level() {
    let _g = serial();
    let (tile_cache, mut store, ground) = roofed(None);
    let red = cube_id(Cube::object(cube::Object::red));
    for z in 0..10 {
        write(
            &mut store,
            [ground[0], ground[1], 5.0 + f64::from(z)],
            OBJECT_COARSEST,
            red,
        );
    }
    let world = World::new(&tile_cache, &store);
    let access = Ground::of(&world);
    let at = pose([ground[0], ground[1], over(15.0)], 18.0, 2.2);
    assert!(in_solid(&at, &access));
    let climbed = climb_pose(at, &access);
    assert!(climbed.polar < at.polar, "{climbed:?}");
    assert!(climbed.polar > core::f64::consts::FRAC_PI_2, "{climbed:?}");
    assert!(!in_solid(&climbed, &access), "{climbed:?}");
}
