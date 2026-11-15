// pxl-core/link/export.test.rs

use super::*;
use crate::abi::edit::Tool;
use crate::abi::scene::SceneContent;
use crate::abi::sensor::Position;
use crate::abi::types::ErrorCode;
use crate::domain::control::camera::state::{camera_state, Intent};
use crate::domain::control::edit::write;
use crate::domain::control::navigation::destination;
use crate::domain::control::param;
use crate::domain::query::test::flat_at_five;
use crate::spec::perf::RING_NAMES;
use pxl_render::gpu::surface::surface_open;
use pxl_spec::asset::marker::{CHAT_TEXT_CHARS, MARKER_TEXT_CHARS};
use pxl_spec::space::coord::world;
use pxl_spec::space::coord::zoom::UNIT_ZOOM;
use pxl_spec::space::layer::octant::Octant;

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
        sensor::pos(Some(Position {
            lon: 0.0,
            lat: off_earth,
            alt: None,
        })),
        sensor::heading(Some(-1.0)),
        sensor::heading(Some(361.0)),
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

/// Whether a frame was asked for, which this takes.
fn take_invalid() -> bool {
    use crate::sys::sched::invalidated;
    use core::future::Future;
    use core::task::{Context, Waker};
    let mut wait = core::pin::pin!(invalidated());
    wait.as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready()
}

/// An input the UI sends, called as the link calls it.
type Input<'a> = &'a dyn Fn() -> Result<(), Errno>;

/// An input that moves the view, what the pointer hovers or the tool asks for
/// its frame, as the core's other methods do, so a still world shows it at
/// once.
#[test]
fn an_input_asks_for_its_frame() {
    use crate::abi::input::{PointerPhase, PointerType};
    use crate::domain::control::camera::fixture::place;
    place([0.0, 0.0, 0.0], 100.0, 1.0, 0.5);
    let pointer = |phase| input::pointer(phase, PointerType::Mouse, 1, 0, 10.0, 10.0);
    let inputs: [(&str, Input); 7] = [
        ("hover", &|| pointer(PointerPhase::Move)),
        ("pointer down", &|| pointer(PointerPhase::Down)),
        ("pressed move", &|| pointer(PointerPhase::Move)),
        ("pointer up", &|| pointer(PointerPhase::Up)),
        ("wheel", &|| input::wheel(1.0)),
        ("pinch", &|| input::pinch(1.1)),
        ("tool", &|| edit::hold(Tool::Pointer, 0, 0)),
    ];
    for (name, input) in inputs {
        take_invalid();
        assert_eq!(input(), Ok(()), "{name}");
        assert!(take_invalid(), "{name} asks for no frame");
    }
}

/// A text is held to its kind's cap and refused whole one past it,
/// `too-long`, a chat line and a pin's draft alike, the refusal asking for no
/// frame; a chat line at its cap is the rule's (pxl-spec `text_fits`), as
/// saying it reads the host's clock. A pin's draft holds its text as it is
/// typed, none at first, and keeps the text it had where it is refused.
#[test]
fn a_text_is_held_to_its_kind_cap() {
    let text = |cnt: u32| "😀".repeat(cnt as usize);
    take_invalid();
    assert_eq!(
        scene::chat(text(CHAT_TEXT_CHARS + 1)),
        Ok(Err(ErrorCode::TooLong))
    );
    assert!(!take_invalid(), "a refused line asks for a frame");

    let [x, y] = flat_at_five();
    write::draft_at(Octant::holding(UNIT_ZOOM, &world::point(x, y, 5.0)));
    assert_eq!(edit::marker_text(String::new()), Ok(Ok(())));
    assert_eq!(edit::marker_text(text(MARKER_TEXT_CHARS)), Ok(Ok(())));
    take_invalid();
    assert_eq!(
        edit::marker_text(text(MARKER_TEXT_CHARS + 1)),
        Ok(Err(ErrorCode::TooLong))
    );
    assert!(!take_invalid(), "a refused draft asks for a frame");
    assert_eq!(
        write::marker_draft(),
        Some(text(MARKER_TEXT_CHARS).as_str())
    );
    write::cancel_draft();
}
