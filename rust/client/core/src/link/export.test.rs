// pxl-core/link/export.test.rs

use super::*;
use crate::abi::edit::Tool;
use crate::abi::overlay::SensorStatus;
use crate::abi::scene::SceneContent;
use crate::domain::control::camera::state::{camera_state, Intent};
use crate::domain::control::navigation::destination;
use crate::domain::control::param;
use crate::domain::control::sensor::device_orientation::{self, OrientationCommand};
use crate::domain::control::sensor::geolocation;
use crate::domain::control::sensor::reading::device_orient_store;
use crate::spec::perf::RING_NAMES;
use pxl_render::gpu::surface::surface_open;

/// Each argument these methods refuse before they touch anything answers
/// EINVAL, whatever else it holds.
#[test]
fn a_refused_argument_answers_its_errno() {
    let off_earth = 181.0;
    assert!(surface_open(1));
    let refused = [
        camera::goto(off_earth, 0.0, None, None, None),
        input::pinch(0.0),
        input::xr_pose(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
        edit::hold(Tool::Item, u32::MAX, 0),
        source::declare(0, Vec::new(), Vec::new()),
        sensor::pos(0.0, off_earth, None, None, None),
        sensor::pos(0.0, 0.0, None, Some(-1.0), None),
        sensor::pos(0.0, 0.0, None, None, Some(361.0)),
        sensor::orient(Some(-1.0), None),
        sensor::orient(None, Some(361.0)),
        scene::draw(1, SceneContent::World, Some(0), None).map(drop),
        scene::draw(1, SceneContent::Character, None, Some(0)).map(drop),
        scene::draw(1, SceneContent::Character, Some(u8::MAX), Some(0)).map(drop),
        perf::ring(RING_NAMES.len() as u32, 0, 1).map(drop),
        perf::ring(0, 1, 1).map(drop),
    ];
    for (k, r) in refused.into_iter().enumerate() {
        assert_eq!(r, Err(Errno::EINVAL), "refusal {k}");
    }
}

/// `sensor.platform` sets the sensors the platform reported, and
/// `orient-unavailable` the compass's status alone: it stops the listener and
/// reads unavailable from then on, the platform's report kept.
#[test]
fn the_platform_and_a_missing_compass_set_what_they_say() {
    assert_eq!(sensor::platform(true, true, false, true), Ok(()));
    assert!(geolocation::platform().supported);
    let p = *device_orientation::platform();
    assert!(p.supported && !p.needs_permission && p.silence_is_loss);
    assert_eq!(sensor::orient_unavailable(), Ok(()));
    assert!(device_orientation::platform().supported, "the report kept");
    assert_eq!(
        device_orientation::take_orient_cmd(),
        Some(OrientationCommand::Stop)
    );
    assert!(!device_orientation::request_orient());
    let store = device_orient_store();
    assert_eq!(
        (store.status, store.is_tracking_orient),
        (SensorStatus::Unavailable, false)
    );
}

fn goto(from: f64, dist: Option<f64>, azimuth: Option<f64>, polar: Option<f64>) -> Intent {
    use crate::domain::control::camera::controller::tracked_end;
    use crate::domain::control::camera::fixture::place;
    tracked_end();
    place([0.0, 0.0, 0.0], from, 1.0, 0.5);
    assert_eq!(camera::goto(127.0, 37.5, dist, azimuth, polar), Ok(()));
    destination::cancel();
    camera_state().intent
}

/// A view field `goto` asks for is applied as the controls would take
/// it, after the camera has come in. A field left out keeps its value, but a
/// view far out still comes in, and a view already nearer stays where it is.
#[test]
fn goto_asks_after() {
    use crate::domain::control::param::DEFAULT_DIST;
    let pi = core::f64::consts::PI;

    let unasked = goto(100.0, None, None, None);
    assert_eq!(
        (unasked.dist, unasked.azimuth, unasked.polar),
        (DEFAULT_DIST, 0.5, 1.0)
    );

    let asked = goto(100.0, Some(50.0), Some(7.0), Some(0.3));
    assert_eq!((asked.dist, asked.polar), (50.0, 0.3));
    assert!((asked.azimuth - (7.0 - 2.0 * pi)).abs() < 1e-12, "wrapped");

    assert_eq!(
        goto(100.0, Some(1e9), None, Some(-1.0)).dist,
        param::MAX_DIST
    );
    assert_eq!(goto(100.0, None, None, Some(-1.0)).polar, 0.0, "clamped");
    assert_eq!(goto(10.0, None, None, None).dist, 10.0, "nearer stays");
}
