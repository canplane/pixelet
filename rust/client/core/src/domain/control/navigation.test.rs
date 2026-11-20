// pxl-core/domain/control/navigation.test.rs

use core::f64::consts::PI;

use xpute_kit::math::scalar::wrap;

use super::*;
use crate::abi::overlay::CameraState;
use crate::abi::sensor::{LocationStatus, Position};
use crate::domain::control::camera::controller::{pivot_to, pose};
use crate::domain::control::camera::fixture::{camera_now, clear_camera, place as put};
use crate::domain::control::param::DEFAULT_DIST;
use crate::domain::eye;
use crate::domain::layer::object::store::Store;
use crate::domain::query::over;
use crate::domain::query::test::{flat_at_five, placed, withhold_at, write};
use crate::domain::setting::{CameraMode, settings_mut};
use crate::domain::view::frame::pose::view_resize;
use pxl_spec::asset::AssetId;
use pxl_spec::asset::cube;
use pxl_spec::bake::constant::OBJECT_COARSEST;
use pxl_spec::space::coord::{q3, world};

/// The app's start: no camera, no eye, no location, and the start has the
/// pivot.
fn start() {
    view_resize(1280.0, 720.0);
    clear_camera();
    eye::set_pose(None);
    *holder() = Holder::Start;
    sensor::set_location_status(LocationStatus::Ready);
    sensor::set_pos(None);
    sensor::set_heading(None);
}

/// A camera standing at (`x`, `y`) on the floor at 5, no one having its
/// pivot.
fn standing(x: f64, y: f64, dist: f64) {
    start();
    *holder() = Holder::NoOne;
    put([x, y, over(5.0)], dist, 1.0, 0.0);
    eye::set_pose(pose());
}

/// The user's location as a fix over (`x`, `y`) tells it.
fn user_at(x: f64, y: f64) {
    let [lon, lat, _] = epsg4326::unproject(&world::point(x, y, 0.0));
    sensor::set_pos(Some(Position {
        lon,
        lat,
        alt_ellipsoid: None,
    }));
}

/// A frame's part of a camera: the navigation's step, the camera's, and the
/// view taking its pose. Returns whether the camera moved or was put.
fn frame() -> bool {
    let was = controller::pivot();
    navigation_step(false);
    let moving = controller::step(1.0 / 60.0);
    eye::set_pose(pose());
    moving || controller::pivot() != was
}

fn settled() {
    let rested = (0..10_000).find(|_| !frame());
    assert!(rested.is_some(), "the camera rests");
}

fn off(x: f64, y: f64) -> f64 {
    let [px, py, _] = camera_now().pivot;
    libm::hypot(px - x, py - y)
}

/// A place is made a point before the camera goes there: a camera never
/// put, sent to a place whose height is not known, is put on its floor in
/// the frame the world answers, and nothing moves after. Then no one has
/// the pivot.
#[test]
fn a_place_is_a_point_before_the_camera_goes() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    start();
    go_to_ground(unknown(&world::point(x + 0.5, y + 0.5, 0.0)));
    assert!(pose().is_none(), "not put by being asked");
    assert!(frame(), "put");
    settled();
    assert_eq!(*holder(), Holder::NoOne, "the place has the pivot no more");
    let to = pivot_to().expect("a camera");
    assert_eq!(q3::to_real(i64::from(to[2])), over(5.0), "its height");
    assert_eq!(
        camera_now().pivot[2],
        over(5.0),
        "on its floor as it is put"
    );
    assert!((0..8).all(|_| !frame()), "and nothing moves after");
}

/// An orbit that is drawn flies to the point, on an animation: the point is
/// made first, it leaves from where it is, it is carried high over the
/// ground between and comes down, and it rests on the point at the distance
/// it had.
#[test]
fn an_orbit_flies_to_the_point() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    standing(x + 4000.5, y + 0.5, DEFAULT_DIST);
    go_to_ground(unknown(&world::point(x + 0.5, y + 0.5, 0.0)));
    navigation_step(false);
    let to = pivot_to().expect("a camera");
    assert_eq!(
        q3::to_real(i64::from(to[2])),
        over(5.0),
        "a point before it moves"
    );
    assert_eq!(state(false), CameraState::Animation);
    assert!(frame(), "it leaves");
    let left = camera_now().pivot[0];
    assert!(
        (left - (x + 4000.5)).abs() < 100.0,
        "from where it is: {left}"
    );
    let ground = pose().expect("a pose").eye[2];
    let mut highest = 0.0f64;
    let rested = (0..600).find(|_| {
        highest = highest.max(pose().expect("a pose").eye[2] - ground);
        !frame()
    });
    assert!(rested.is_some(), "the flight ends");
    assert!(
        (700.0..900.0).contains(&highest),
        "the camera was carried about a fifth of the way high: {highest}"
    );
    assert_eq!(pivot_to(), Some(to), "the point did not change on the way");
    let at = camera_now();
    assert!(off(x + 0.5, y + 0.5) < 1e-6);
    assert_eq!((at.pivot[2], at.dist), (over(5.0), DEFAULT_DIST));
}

/// A place waits until the world holds the leaves over it, whatever coarser
/// stands in: the camera is not sent, the terrain is asked for its leaves,
/// and once the place is a point the camera goes.
#[test]
fn a_place_waits_for_its_leaves() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    start();
    withhold_at([x + 0.5, y + 0.5]);
    let at = unknown(&world::point(x + 0.5, y + 0.5, 0.0));
    go_to_ground(at);
    assert!(!frame() && !frame(), "nothing is put");
    assert_eq!(
        destination(),
        Some(at),
        "the place waits, its leaves asked for"
    );
    assert!(pose().is_none(), "and the camera is not drawn");
    placed(Store::default());
    flat_at_five();
    assert!(frame(), "put once the leaves are there");
    settled();
    assert_eq!(*holder(), Holder::NoOne);
}

/// A place named with a height is a point on the floor nearest that height:
/// of the ground and a cube over it, the one the height is nearer. A cell a
/// tap names is a point on the floor under it.
#[test]
fn a_height_picks_the_floor_nearest_it() {
    let [x, y] = flat_at_five();
    let mut store = Store::default();
    let red = cube::Object::Red as AssetId;
    write(
        &mut store,
        [x.floor(), y.floor(), 9.0],
        OBJECT_COARSEST,
        red,
    );
    placed(store);
    for (named, floor) in [(6.0, 5.0), (10.6, 10.0)] {
        start();
        go_to_ground(world::point(x + 0.5, y + 0.5, named));
        assert!(frame(), "{named}: put");
        assert_eq!(
            camera_now().pivot[2],
            over(floor),
            "{named}: the nearer floor"
        );
    }
    start();
    go_to_floor_under(world::point(x + 0.5, y + 0.5, 8.5));
    assert!(frame(), "put");
    assert_eq!(camera_now().pivot[2], over(5.0), "on the floor under it");
}

/// The camera sent to a place with a view far out: the view comes in to the
/// default distance on the animation that takes the camera there, not
/// before it leaves. A view already nearer stays as near.
#[test]
fn a_view_far_out_comes_in_as_it_goes() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    // A place apart for each, since a place once a point is asked no more.
    for (from, to, py) in [(100.0, DEFAULT_DIST, y + 0.5), (10.0, 10.0, y + 1.5)] {
        standing(x + 200.5, y + 0.5, from);
        withhold_at([x + 0.5, py]);
        go_to_ground(unknown(&world::point(x + 0.5, py, 0.0)));
        assert!(!frame(), "{from}: it waits, and nothing moves");
        assert_eq!(camera_now().dist, from, "{from}: the view as it was");
        placed(Store::default());
        flat_at_five();
        assert!(frame(), "{from}: it leaves");
        settled();
        assert!(off(x + 0.5, py) < 1e-6, "{from}: there");
        assert_eq!(camera_now().dist, to, "{from}: at the distance");
    }
}

/// Tracking: the user's location has the pivot, and the camera goes after
/// it, idle all the while, at the distance the user set, sent nowhere. With
/// the heading the view turns to it; a rotation by hand ends the turning
/// alone and a zoom keeps the mode; the button's end turns the view to
/// north and leaves the camera where it is.
#[test]
fn tracking_goes_after_the_location() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    standing(x + 0.5, y + 0.5, 16.0);
    user_at(x + 4.5, y + 0.5);
    assert_eq!(
        destination(),
        None,
        "no one has the pivot: no place is made a point"
    );

    track(TrackingMode::Follow);
    assert_eq!(destination(), user_location(), "the location is");
    assert!(frame(), "the camera goes after it");
    assert_eq!(state(false), CameraState::Idle, "sent nowhere");
    settled();
    assert!(off(x + 4.5, y + 0.5) < 0.01, "at the location");
    assert_eq!(camera_now().pivot[2], over(5.0), "on its floor");

    user_at(x + 6.5, y + 1.5);
    let mut moved = false;
    for _ in 0..10_000 {
        let on = frame();
        assert_eq!(state(false), CameraState::Idle, "no animation");
        moved |= on;
        if !on {
            break;
        }
    }
    assert!(moved && off(x + 6.5, y + 1.5) < 0.01, "it followed");
    controller::zoom(0.25);
    settled();
    user_at(x + 7.5, y + 1.5);
    settled();
    assert!(off(x + 7.5, y + 1.5) < 0.01, "followed on");
    assert_eq!(camera_now().dist, 64.0, "at the distance the user set");
    controller::zoom(8.0);
    settled();

    track(TrackingMode::FollowWithHeading);
    for k in 1..=10 {
        let heading = 1.6 * f64::from(k);
        sensor::set_heading(Some(heading));
        settled();
        let off = wrap(camera_now().azimuth - heading.to_radians(), -PI, PI);
        assert!(
            off.abs() < 1e-9,
            "{heading}° rests {} short",
            off.to_degrees()
        );
    }
    stop_tracking_heading();
    assert_eq!(tracking_mode(), TrackingMode::Follow, "tracked on");
    sensor::set_heading(Some(180.0));
    settled();
    assert!((camera_now().azimuth - 16.0f64.to_radians()).abs() < 1e-9);
    assert_eq!(camera_now().dist, 8.0, "a zoom kept the mode");

    track(TrackingMode::None);
    settled();
    assert_eq!(
        (tracking_mode(), camera_now().azimuth),
        (TrackingMode::None, 0.0)
    );
    user_at(x + 8.5, y + 0.5);
    settled();
    assert!(off(x + 7.5, y + 1.5) < 0.01, "and the camera stays");
    assert_eq!(destination(), None);
}

/// One has the pivot at a time, whoever asked last: a place, tracking, the
/// hand. Taking it ends an animation under way in place, and what was
/// tracked is tracked no more.
#[test]
fn who_asked_last_has_the_pivot() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    standing(x + 0.5, y + 0.5, 16.0);
    user_at(x + 4.5, y + 0.5);
    let marker = world::point(x + 300.5, y + 0.5, over(5.0));

    go_to_ground(marker);
    assert!(frame() && frame(), "on its way to the marker");
    assert_eq!(state(false), CameraState::Animation);
    track(TrackingMode::Follow);
    assert_eq!(
        state(false),
        CameraState::Idle,
        "the animation ended in place"
    );
    assert_eq!(destination(), user_location(), "tracking has the pivot");
    settled();
    assert!(off(x + 4.5, y + 0.5) < 0.01, "and the camera went after it");

    go_to_ground(marker);
    assert_eq!(tracking_mode(), TrackingMode::None, "ended by the going");
    assert_eq!(destination(), Some(marker));
    take_by_hand();
    assert_eq!(destination(), None, "the hand has it: nothing waits");
    settled();
    assert!(off(x + 4.5, y + 0.5) < 0.01, "and the camera went nowhere");
}

/// The start has the pivot until someone takes it: the user's location,
/// coming after the camera was put elsewhere, takes the camera over with a
/// warp, at the distance and the facing it has, and then no one has it.
/// Once someone has taken it, a location that comes is not the start's. A
/// camera never put is put all the same: at the fallback where its holder
/// wants it nowhere.
#[test]
fn the_start_has_the_pivot_until_someone_takes_it() {
    let [x, y] = flat_at_five();
    placed(Store::default());

    standing(x + 30.5, y + 0.5, 300.0);
    *holder() = Holder::Start;
    assert_eq!(
        destination(),
        None,
        "no location: the camera stands where it is"
    );
    user_at(x + 0.5, y + 0.5);
    assert_eq!(destination(), user_location());
    assert!(frame(), "taken over");
    assert!(off(x + 0.5, y + 0.5) < 0.01, "there at once");
    assert_eq!(
        (camera_now().dist, state(false)),
        (300.0, CameraState::Idle)
    );
    assert_eq!(*holder(), Holder::NoOne, "the start is over");
    user_at(x + 3.5, y + 0.5);
    assert!(!frame(), "a later fix is no one's");

    standing(x + 30.5, y + 0.5, 300.0);
    *holder() = Holder::Start;
    take_by_hand();
    user_at(x + 0.5, y + 0.5);
    assert_eq!(
        destination(),
        None,
        "taken by the hand before the location came"
    );

    start();
    let fallback = surface(&FALLBACK_G4326);
    assert_eq!(destination(), Some(fallback), "never put, no location");
    track(TrackingMode::Follow);
    assert_eq!(destination(), Some(fallback), "tracking with no fix yet");
    user_at(x + 0.5, y + 0.5);
    assert!(frame(), "put where the location is");
    assert!(off(x + 0.5, y + 0.5) < 0.01);
    track(TrackingMode::None);
}

/// The camera does one thing at a time. While a hand holds the view a place
/// waits with its holder, and is sent once the hand lets go; on its way
/// there, an animation, the camera takes no gesture and its mode is not
/// changed; arrived, it is idle again.
#[test]
fn the_camera_does_one_thing_at_a_time() {
    let [x, y] = flat_at_five();
    placed(Store::default());
    standing(x + 0.5, y + 0.5, 16.0);
    assert_eq!(state(false), CameraState::Idle);
    assert_eq!(state(true), CameraState::Gesture);
    go_to_ground(world::point(x + 300.5, y + 0.5, over(5.0)));
    navigation_step(true);
    assert!(matches!(*holder(), Holder::Place(..)), "the place waits");
    assert!(takes_gesture(), "and the hand is taken meanwhile");
    assert!(frame(), "sent once the hand lets go");
    assert_eq!(state(true), CameraState::Animation);
    assert!(!takes_gesture(), "no gesture on its way");
    settings_mut().camera_mode = CameraMode::FirstPerson;
    assert!(frame());
    assert_eq!(
        controller::mode(),
        CameraMode::Orbit,
        "nor a change of mode"
    );
    settings_mut().camera_mode = CameraMode::Orbit;
    settled();
    assert!(!frame());
    assert_eq!(state(false), CameraState::Idle, "arrived");
    assert!(off(x + 300.5, y + 0.5) < 1e-6);
    assert!(takes_gesture());
}
