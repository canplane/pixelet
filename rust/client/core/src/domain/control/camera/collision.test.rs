// pxl-core/domain/control/camera/collision.test.rs
//
// Tests of the dive under a roof, the zoom out held under it, and the climb.
// The ground is flat at height five. A cube over it makes a roof, and a slab
// of cubes makes a tunnel's roof.

use super::*;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed, write};
use pxl_content::cave::hollow;
use pxl_spec::asset::cube::{self};
use pxl_spec::asset::AssetId;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::share::FULL;
use pxl_spec::space::coord::zoom::LEAF_CELL_ZOOM;

fn pose(pivot: V3, dist: f64, polar: f64) -> ViewPose {
    ViewPose {
        pivot,
        dist,
        polar,
        azimuth: 0.0,
    }
}

/// A pose whose eye is level with `pivot` and `dist` east of it.
fn east(pivot: V3, dist: f64) -> ViewPose {
    ViewPose {
        pivot,
        dist,
        polar: core::f64::consts::FRAC_PI_2,
        azimuth: -core::f64::consts::FRAC_PI_2,
    }
}

/// Writes a red cube at `at`, one cell at the objects' coarsest zoom.
fn cube_at(store: &mut Store, at: V3) {
    write(store, at, OBJECT_COARSEST, cube::Object::red as AssetId);
}

/// The ground at five, a roof whose underside is `roof` above the ground
/// when `roof` is given, and the pivot standing on the ground under it.
fn roofed(roof: Option<f64>) -> (Store, V3) {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    if let Some(roof) = roof {
        cube_at(&mut store, [x, y, 5.0 + roof]);
    }
    (store, [x + 0.5, y + 0.5, over(5.0)])
}

/// Zooms from 16 to `to` at the pitch `polar` and returns the distance the
/// eye dives to, if it dives.
fn dive_to(roof: Option<f64>, to: f64, polar: f64) -> Option<f64> {
    let (store, pivot) = roofed(roof);
    let world = placed(store);
    let access = Ground::of(&world);
    let to = pose(pivot, to, polar);
    dive(&to, &pose(pivot, 16.0, polar), &access).map(|p| p.dist)
}

/// Zooms straight down from `from` to `to` under `climb` and returns the
/// distance it reaches.
fn climbed_to(roof: Option<f64>, from: f64, to: f64) -> f64 {
    let (store, pivot) = roofed(roof);
    let world = placed(store);
    let access = Ground::of(&world);
    climb_step(&pose(pivot, to, 0.0), &pose(pivot, from, 0.0), &access).dist
}

#[test]
fn dives_under_roof() {
    crate::on_core(|| {
        let under = 8.0 - over(5.0) - NEAR;
        assert_eq!(dive_to(Some(3.0), 10.0, 0.0), Some(under));
        // A zoom asked nearer than the roof stands where it was asked.
        assert_eq!(dive_to(Some(3.0), 2.2, 0.0), Some(2.2));
    });
}

/// A zoom short of straight down, at whatever pitch the climb stopped,
/// dives as well, along its own line.
#[test]
fn dives_slanted() {
    crate::on_core(|| {
        let polar: f64 = 0.1;
        let under = (8.0 - over(5.0)) / libm::cos(polar) - NEAR;
        let dist = dive_to(Some(3.0), 10.0, polar).unwrap();
        assert!((dist - under).abs() < 1e-6);
    });
}

#[test]
fn dive_held_back() {
    crate::on_core(|| {
        // The line passes beside the roof.
        assert_eq!(dive_to(Some(3.0), 10.0, 0.3), None);
        assert_eq!(dive_to(None, 10.0, 0.0), None);
    });
}

/// Under a roof nearer the pivot than the orbit can come, the zoom goes to
/// the orbit's nearest distance.
#[test]
fn dives_into_low_roof() {
    crate::on_core(|| {
        assert_eq!(dive_to(Some(2.0), 10.0, 0.0), Some(MIN_DIST));
        assert_eq!(dive_to(Some(1.0), 10.0, 0.0), Some(MIN_DIST));
    });
}

/// A zoom out goes as far as it can. Under a roof it goes up to the roof,
/// and at the nearest in the roof's lowest cell it does not go out at all.
/// From over the roof, or with nothing over the pivot, it goes where it is
/// asked. A zoom in whose way hit a slab goes under it, whether it starts
/// over the slab or in it. A zoom out that starts in the slab stops.
#[test]
fn zoom_out_does_not_go_through() {
    crate::on_core(|| {
        let dist = climbed_to(Some(3.0), MIN_DIST, 10.0);
        assert!((MIN_DIST..=8.0 - over(5.0)).contains(&dist), "{dist}");
        assert_eq!(climbed_to(Some(2.0), MIN_DIST, 10.0), MIN_DIST);
        assert_eq!(climbed_to(Some(3.0), 16.0, 20.0), 20.0);
        assert_eq!(climbed_to(None, MIN_DIST, 10.0), 10.0);
        let (store, pivot) = tunnel(3.0, 2);
        let world = placed(store);
        let access = Ground::of(&world);
        let to = climb_step(&pose(pivot, 3.8, 0.0), &pose(pivot, 16.0, 0.0), &access);
        assert_eq!(to.dist, 8.0 - over(5.0) - NEAR);
        // From inside the slab, a zoom out stops where it stands and a zoom in
        // dives.
        let out = climb_step(&pose(pivot, 4.2, 0.0), &pose(pivot, 3.8, 0.0), &access);
        assert_eq!(out.dist, 3.8);
        let down = climb_step(&pose(pivot, 3.4, 0.0), &pose(pivot, 3.8, 0.0), &access);
        assert_eq!(down.dist, 8.0 - over(5.0) - NEAR);
    });
}

/// The eye climbs over a low wall on its way, for a zoom in and a zoom out
/// alike, and gets the distance asked, whether it starts level or above.
/// Under a slab that meets the wall, the climb hits the slab on its way up.
/// Then a zoom out stops short, and a zoom in dives through the wall.
#[test]
fn climb_or_dive_or_stop() {
    crate::on_core(|| {
        let wall = |store: &mut Store, pivot: V3, top: f64| {
            for dy in -3..=3 {
                for z in [5.0, 6.0].into_iter().filter(|&z| z < top) {
                    cube_at(store, [pivot[0] + 3.0, pivot[1] + f64::from(dy), z]);
                }
            }
        };

        let (mut store, pivot) = roofed(None);
        wall(&mut store, pivot, 6.0);
        let world = placed(store);
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

        let (mut store, pivot) = tunnel(2.0, 1);
        wall(&mut store, pivot, 7.0);
        let world = placed(store);
        let access = Ground::of(&world);
        let to = east(pivot, 3.2);
        let stopped = climb_step(&to, &east(pivot, 2.4), &access).dist;
        assert!((2.4..3.2).contains(&stopped), "{stopped}");
        // Where the climb clears the slab's corner, it climbs. Where the slab is
        // too thick, a zoom in dives through the wall.
        assert_eq!(climb_step(&to, &east(pivot, 4.2), &access), to);
        let (mut store, pivot) = tunnel(2.0, 2);
        wall(&mut store, pivot, 7.0);
        let world = placed(store);
        let access = Ground::of(&world);
        let to = east(pivot, 3.2);
        let dived = climb_step(&to, &east(pivot, 4.2), &access).dist;
        assert!((dived - (2.5 - NEAR)).abs() < 1e-9, "{dived}");
    });
}

/// Over open ground, beyond the nearest, the eye keeps one cell over its
/// floor. At the nearest it stands as asked.
#[test]
fn open_ground_keeps_a_cell() {
    crate::on_core(|| {
        let (store, pivot) = roofed(None);
        let world = placed(store);
        let access = Ground::of(&world);
        let level = core::f64::consts::FRAC_PI_2;
        assert_eq!(
            climb_pose(pose(pivot, MIN_DIST, level), &access).polar,
            level
        );
        // At the nearest below the pivot's level, a tilt in the air goes as
        // asked. A tilt into the floor stops short of it from any pitch, and the
        // eye stays out of the floor.
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
    });
}

/// At the orbit's nearest, the eye turns above the pivot's level whatever is
/// in the way. Turned into a wall level with the pivot and a cell beside it,
/// or into a thick block round the pivot, the climb keeps the asked pitch.
/// Beyond the nearest, and below the pivot's level, the eye climbs and stops
/// as it does anywhere.
#[test]
fn nearest_turns_through() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
        for dx in [-2.5, -1.5, 1.5, 2.5] {
            for dy in [-2.5, -1.5, -0.5, 0.5, 1.5] {
                for z in [5.0, 6.0, 7.0] {
                    cube_at(&mut store, [pivot[0] + dx, pivot[1] + dy, z]);
                }
            }
        }
        let world = placed(store);
        let access = Ground::of(&world);
        for polar in [0.4, 1.0, core::f64::consts::FRAC_PI_2] {
            for azimuth in [-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2] {
                let at = ViewPose {
                    polar,
                    azimuth,
                    ..east(pivot, MIN_DIST)
                };
                let climbed = climb_pose(at, &access);
                assert_eq!(climbed, at);
            }
        }
        let at = east(pivot, 3.0);
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
        // A tilt asked below the level into a wall stays where it stands.
        let level = ViewPose {
            dist: MIN_DIST,
            ..at
        };
        let past = ViewPose {
            polar: 2.2,
            ..level
        };
        assert_eq!(climb_step(&past, &level, &access), level);
    });
}

/// A zoom all the way in, tilted below the level, leaves the eye in the
/// ground under the pivot. From there it still turns, tilts back and zooms
/// out. Only going deeper is refused.
#[test]
fn the_nearest_under_the_level_is_no_trap() {
    crate::on_core(|| {
        let (store, pivot) = roofed(None);
        let world = placed(store);
        let access = Ground::of(&world);
        let at = ViewPose {
            polar: core::f64::consts::FRAC_PI_2 + 0.4,
            ..pose(pivot, MIN_DIST, 0.0)
        };
        assert!(in_solid(&at, &access));
        let turned = ViewPose { azimuth: 1.0, ..at };
        assert_eq!(climb_step(&turned, &at, &access), turned);
        let back = ViewPose {
            polar: at.polar - 0.01,
            ..at
        };
        assert_eq!(climb_step(&back, &at, &access), back);
        let out = ViewPose { dist: 10.0, ..at };
        assert_eq!(climb_step(&out, &at, &access), out);
        let deeper = ViewPose {
            polar: at.polar + 0.1,
            ..at
        };
        assert_eq!(climb_step(&deeper, &at, &access), at);
    });
}

/// `stop` tests only the straight line. Into a low wall it stops short from
/// either side, for a zoom in and a zoom out alike.
#[test]
fn stop_goes_straight() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
        for dy in -3..=3 {
            cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + f64::from(dy), 5.0]);
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let dist_at = |from: f64, to: f64| {
            way_hit(&east(pivot, from), &east(pivot, to), |at| *at, &access)
                .map(|t| from + (to - from) * t)
        };
        assert!(dist_at(2.3, 6.0).unwrap() < 3.0);
        assert!(dist_at(6.0, 2.8).unwrap() > 3.0);
        assert_eq!(dist_at(2.3, 2.45), None);
    });
}

/// The ground at five and a tunnel's roof over the pivot. The roof is a slab
/// seven cells across and `thick` cells thick, with its underside `roof`
/// above the ground.
fn tunnel(roof: f64, thick: u32) -> (Store, V3) {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    for dz in 0..thick {
        for dx in -3..=3 {
            for dy in -3..=3 {
                let at = [
                    x + f64::from(dx),
                    y + f64::from(dy),
                    5.0 + roof + f64::from(dz),
                ];
                cube_at(&mut store, at);
            }
        }
    }
    (store, [x + 0.5, y + 0.5, over(5.0)])
}

/// In the roof's lowest cell, where the eye may stand, the climb asks for no
/// rise. A tilt there is drawn as asked and is not held straight down.
#[test]
fn climb_stands_in_roof_cell() {
    crate::on_core(|| {
        let (store, pivot) = tunnel(2.0, 1);
        let world = placed(store);
        let access = Ground::of(&world);
        let climbed = climb_pose(pose(pivot, MIN_DIST, 0.3), &access);
        assert_eq!(climbed.polar, 0.3);
    });
}

/// In a roof thicker than its lowest cell, even straight down does not
/// clear it, so the climb keeps the asked pitch, on the way in and at rest.
#[test]
fn climb_fails_under_roof() {
    crate::on_core(|| {
        let (store, pivot) = tunnel(3.0, 2);
        let world = placed(store);
        let access = Ground::of(&world);
        // The eye is in the slab's upper cell, between heights 9 and 10.
        let climbed = climb_pose(pose(pivot, 3.8, 0.1), &access);
        assert_eq!(climbed.polar, 0.1);
    });
}

/// Over open ground the climb steepens. A block three cells across and
/// three high round the eye, deeper than one cell, lifts the eye to clear the
/// block's top.
#[test]
fn climb_clears_block() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
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
        let access = Ground::of(&world);
        let at = east(pivot, 3.0);
        let climbed = climb_pose(at, &access);
        assert!(climbed.polar < at.polar, "{climbed:?}");
        assert!(!in_solid(&climbed, &access), "{climbed:?}");
    });
}

/// Beyond the nearest, the eye does not stand in a wall. Asked level with
/// the pivot into a wall one cell thick and three cells east, the eye climbs
/// over the wall instead of into it.
#[test]
fn climbs_out_of_wall_with_room() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
        for dy in [-1.5, -0.5, 0.5, 1.5] {
            cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + dy, 5.0]);
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let at = east(pivot, 3.0);
        assert!(in_solid(&at, &access));
        let climbed = climb_pose(at, &access);
        assert!(climbed.polar < at.polar, "{climbed:?}");
        assert!(!in_solid(&climbed, &access), "{climbed:?}");
    });
}

/// The rise is read under the eye itself, however far the eye is. Under a
/// slab, an eye in a wall that meets the slab rises by the floor under it.
#[test]
fn climb_rises_under_eye() {
    crate::on_core(|| {
        let (mut store, pivot) = tunnel(3.0, 1);
        for dy in [-1.5, -0.5, 0.5, 1.5] {
            for z in [5.0, 6.0, 7.0] {
                cube_at(&mut store, [pivot[0] + 3.0, pivot[1] + dy, z]);
            }
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let at = east(pivot, 3.0);
        assert!(in_solid(&at, &access));
        let rise = climb_rise(&at, &access).expect("a rise under the eye");
        assert!(rise > 0.0, "{rise}");
    });
}

/// Ground that two passes do not climb out of is not a roof, so the climbed
/// pitch is kept, not the asked one. The eye is level with the pivot, 40
/// out, in ground that rises a cell for each cell toward the pivot. Each
/// pass steepens the pitch and draws the eye in over higher ground, so two
/// passes do not clear it, yet straight down does.
#[test]
fn slope_keeps_pitch_climbed() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
        // From 20 to 40 cells east, each column is 60 cells high less its
        // distance.
        for dx in 20..=40 {
            for z in 0..60 - dx {
                let at = [pivot[0] + f64::from(dx), pivot[1] - 0.5, 5.0 + f64::from(z)];
                cube_at(&mut store, at);
            }
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let at = east(pivot, 40.0);
        assert!(in_solid(&at, &access));
        let climbed = climb_pose(at, &access);
        assert!(climbed.polar < at.polar, "{climbed:?}");
    });
}

/// The pivot stands on a tower ten cells high, under a roof. The eye is
/// tilted below the level and out to where it lies under the ground, inside
/// one of the seed's caves. With the pivot roofed, a cave's air does not
/// count as ground, so the climb does not lift the eye.
#[test]
fn a_cave_is_not_climbed_out_of() {
    crate::on_core(|| {
        let (mut store, ground) = roofed(None);
        for z in (0..10).chain([12]) {
            cube_at(&mut store, [ground[0], ground[1], 5.0 + f64::from(z)]);
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let pivot = [ground[0], ground[1], over(15.0)];
        // A pose whose eye lies under the ground, in a leaf cell the seed's caves
        // hollow out.
        let at = (0..90)
            .flat_map(|a| (0..50).flat_map(move |d| (0..20).map(move |p| (a, d, p))))
            .map(|(a, d, p)| ViewPose {
                azimuth: core::f64::consts::TAU * f64::from(a) / 90.0,
                ..pose(pivot, 10.0 + f64::from(d), 1.9 + 0.05 * f64::from(p))
            })
            .find(|at| {
                let e = eye(at).0;
                e[2] < 4.0 && hollow((LEAF_CELL_ZOOM, e.map(|v| v.floor() as i32))) >= FULL
            })
            .expect("an eye in a cave under the ground");
        assert!(!in_solid(&at, &access), "{at:?}");
        let climbed = climb_pose(at, &access);
        assert_eq!(climbed.polar, at.polar, "{climbed:?}");
    });
}

/// With the sky open over the pivot, the eye does not settle in a hollow
/// inside a hill. Zooming out and back in over the hill keeps the eye over
/// it, at the distance asked. With the pivot inside the hollow, the eye stays
/// in it as asked.
#[test]
fn a_hollow_in_a_hill() {
    crate::on_core(|| {
        let (mut store, pivot) = roofed(None);
        for dx in 8..=40 {
            for dy in -6..=6 {
                for z in (0..12).filter(|z| !((18..=30).contains(&dx) && (3..=7).contains(z))) {
                    let at = [
                        pivot[0] + f64::from(dx),
                        pivot[1] + f64::from(dy),
                        5.0 + f64::from(z),
                    ];
                    cube_at(&mut store, at);
                }
            }
        }
        let world = placed(store);
        let access = Ground::of(&world);
        // A pose east of the pivot, a little above its level.
        let raised = |pivot: V3, dist: f64| ViewPose {
            polar: 1.45,
            ..east(pivot, dist)
        };
        let over_hill = |dist: f64| {
            let e = eye(&climb_pose(raised(pivot, dist), &access)).0;
            let x = e[0] - pivot[0];
            assert!(!(8.5..40.0).contains(&x) || e[2] >= 17.0, "{dist} {e:?}");
        };
        let mut d: f64 = 6.0;
        while d < 60.0 {
            let to = raised(pivot, d * 1.1);
            assert_eq!(climb_step(&to, &raised(pivot, d), &access), to);
            over_hill(to.dist);
            d = to.dist;
        }
        while d > 6.0 {
            let to = raised(pivot, d / 1.1);
            assert_eq!(climb_step(&to, &raised(pivot, d), &access), to);
            over_hill(to.dist);
            d = to.dist;
        }
        let inside = [pivot[0] + 22.0, pivot[1], over(8.0)];
        let at = raised(inside, 4.0);
        assert!(!in_solid(&at, &access));
        assert!(eye(&climb_pose(at, &access)).0[2] < 13.0);
    });
}

/// The pivot stands on a tower ten cells high. The eye is tilted below the
/// level, 18 out, and sunk in the ground there, clear of the seed's caves.
/// The floor under the eye lies below the pivot, so the climb lifts the eye
/// onto that floor and keeps it below the level.
#[test]
fn climbs_past_level() {
    crate::on_core(|| {
        let (mut store, ground) = roofed(None);
        for z in 0..10 {
            cube_at(&mut store, [ground[0], ground[1], 5.0 + f64::from(z)]);
        }
        let world = placed(store);
        let access = Ground::of(&world);
        let at = pose([ground[0], ground[1], over(15.0)], 18.0, 2.2);
        assert!(in_solid(&at, &access));
        let climbed = climb_pose(at, &access);
        assert!(climbed.polar < at.polar, "{climbed:?}");
        assert!(climbed.polar > core::f64::consts::FRAC_PI_2, "{climbed:?}");
        assert!(!in_solid(&climbed, &access), "{climbed:?}");
    });
}
