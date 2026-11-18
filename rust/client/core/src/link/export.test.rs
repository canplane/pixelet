// pxl-core/link/export.test.rs

use super::*;
use crate::abi::edit::Tool;
use crate::abi::perf::Rows;
use crate::abi::scene::SceneContent;
use crate::abi::sensor::Position;
use crate::abi::types::ErrorCode;
use crate::domain::control::camera::state::{Intent, camera_state};
use crate::domain::control::edit::write;
use crate::domain::control::navigation::destination;
use crate::domain::query::test::flat_at_five;
use crate::spec::perf::TABLE_NAMES;
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
        camera::goto(off_earth, 0.0),
        input::zoom(0.0),
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
        perf::table(TABLE_NAMES.len() as u32, 0, 0).map(drop),
    ];
    for (k, r) in refused.into_iter().enumerate() {
        assert_eq!(r, Err(Errno::EINVAL), "refusal {k}");
    }
}

/// A table's read answers the rows from a sequence on, no more than asked
/// for or than a page holds, and says where they start: past the sequence
/// asked from where the table dropped that row, at the table's end where the
/// sequence is its end, and as many rows before its end as asked for where
/// the sequence is past it.
#[test]
fn a_table_is_read_by_sequence() {
    use crate::perf::frame::FrameSample;
    use crate::perf::record::{perf_record_frame, perf_recording_start};
    use crate::spec::perf::{frame, limit};
    use crate::sys::host::set_clock;
    const FRAMES: u32 = 0;
    let row_sz = size_of::<FrameSample>();
    let page = limit::PAGE_SZ as usize / row_sz;
    set_clock(0.0);
    assert!(perf_recording_start());
    let start = perf::table(FRAMES, u64::MAX, 0).unwrap().end;
    // A frame's record lands when the next frame closes its interval; the
    // row of sequence `start + k` holds `k` as its time.
    let land = |range: core::ops::Range<u32>| {
        for k in range {
            let s = FrameSample {
                t: f64::from(k),
                ..FrameSample::default()
            };
            perf_record_frame(s, 0.0);
        }
    };
    let times = |r: &Rows| -> Vec<f64> {
        r.bytes
            .chunks(row_sz)
            .map(|row| {
                let at = 8 * frame::T;
                f64::from_ne_bytes(row[at..at + 8].try_into().unwrap())
            })
            .collect()
    };
    land(0..6);
    let r = perf::table(FRAMES, start, 2).unwrap();
    assert_eq!(
        (r.first, r.end, times(&r)),
        (start, start + 5, vec![0.0, 1.0])
    );
    let r = perf::table(FRAMES, start + 2, u32::MAX).unwrap();
    assert_eq!((r.first, times(&r)), (start + 2, vec![2.0, 3.0, 4.0]));
    let r = perf::table(FRAMES, start + 5, u32::MAX).unwrap();
    assert_eq!((r.first, r.end, times(&r)), (start + 5, start + 5, vec![]));
    let r = perf::table(FRAMES, u64::MAX, 2).unwrap();
    assert_eq!((r.first, times(&r)), (start + 3, vec![3.0, 4.0]));
    let r = perf::table(FRAMES, start + 9, u32::MAX).unwrap();
    assert_eq!((r.first, times(&r).len()), (start, 5), "every row it has");

    // A full table drops its oldest rows as the next lands, and a read from a
    // row dropped starts at the oldest the table has.
    let mut next = 6;
    while perf::table(FRAMES, start, 0).unwrap().first == start {
        land(next..next + 1);
        next += 1;
    }
    let r = perf::table(FRAMES, start + 5, u32::MAX).unwrap();
    assert!(r.first > start + 5, "the row asked from was dropped");
    assert_eq!(r.end, start + u64::from(next) - 1);
    assert_eq!(r.bytes.len(), page * row_sz, "a page at most");
    assert_eq!(times(&r)[0], (r.first - start) as f64);
}

fn goto(from: f64) -> Intent {
    use crate::domain::control::camera::controller::tracked_end;
    use crate::domain::control::camera::fixture::place;
    tracked_end();
    place([0.0, 0.0, 0.0], from, 1.0, 0.5);
    assert_eq!(camera::goto(127.0, 37.5), Ok(()));
    destination::cancel();
    camera_state().intent
}

/// `goto` keeps the camera's azimuth and polar, and a view far out comes in
/// to the default distance, while a view already nearer stays where it is.
#[test]
fn goto_keeps_the_view_and_comes_in() {
    use crate::domain::control::param::DEFAULT_DIST;
    let far = goto(100.0);
    assert_eq!((far.dist, far.azimuth, far.polar), (DEFAULT_DIST, 0.5, 1.0));
    assert_eq!(goto(10.0).dist, 10.0, "nearer stays");
}

/// Whether a frame was asked for, which this takes.
fn take_invalid() -> bool {
    use crate::sys::sched::invalidated;
    use core::task::{Context, Waker};
    let mut wait = core::pin::pin!(invalidated());
    wait.as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready()
}

/// An input the UI sends, called as the link calls it.
type Input<'a> = &'a dyn Fn() -> Result<(), Errno>;

/// How many times a frame was asked for as a method, which this takes.
fn take_method_asks() -> u32 {
    use crate::spec::step::InvalidationReason;
    use crate::sys::sched::take_invalidations;
    take_invalidations()[InvalidationReason::Method as usize]
}

/// A method of the UI's that changes what the core holds asks for its frame
/// as a method, once, whatever its state asks besides, so a still world shows
/// it at once.
#[test]
fn a_method_applied_asks_for_its_frame() {
    use crate::abi::camera::Tracking;
    use crate::abi::{camera as cam, edit as ed, input as inp, sensor as sen};
    use crate::domain::control::camera::fixture::place;
    place([0.0, 0.0, 0.0], 100.0, 1.0, 0.5);
    let methods: [(&str, Input); 15] = [
        ("hover", &|| inp::Guest::hover(&Core, 10.0, 10.0)),
        ("drag", &|| inp::Guest::drag(&Core, true)),
        ("pan", &|| inp::Guest::pan(&Core, 1.0, 1.0)),
        ("turn", &|| inp::Guest::turn(&Core, 0.1)),
        ("tilt", &|| inp::Guest::tilt(&Core, 0.1)),
        ("zoom", &|| inp::Guest::zoom(&Core, 1.1)),
        ("drag end", &|| inp::Guest::drag(&Core, false)),
        ("tap", &|| inp::Guest::tap(&Core, 10.0, 10.0)),
        ("out", &|| inp::Guest::out(&Core)),
        ("xr pose", &|| {
            inp::Guest::xr_pose(&Core, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0)
        }),
        ("xr pose end", &|| inp::Guest::xr_pose_end(&Core)),
        ("tool", &|| ed::Guest::hold(&Core, Tool::Pointer, 0, 0)),
        ("follow", &|| cam::Guest::track(&Core, Tracking::Follow)),
        ("goto", &|| cam::Guest::goto(&Core, 127.0, 37.5)),
        ("pos", &|| sen::Guest::pos(&Core, None)),
    ];
    for (name, method) in methods {
        take_invalid();
        take_method_asks();
        assert_eq!(method(), Ok(()), "{name}");
        assert!(take_invalid(), "{name} asks for no frame");
        assert_eq!(take_method_asks(), 1, "{name}");
    }
    cam::Guest::track(&Core, Tracking::Unfollow).unwrap();
    destination::cancel();
    // The methods of the other answers and of the other interfaces.
    use crate::abi::sensor::LocationStatus;
    use crate::abi::{scene as sc, setting as st};
    assert!(surface_open(2));
    take_method_asks();
    assert_eq!(sen::Guest::location(&Core, LocationStatus::Ready), Ok(()));
    assert_eq!(take_method_asks(), 1, "location");
    let drawn = sc::Guest::draw(&Core, 2, SceneContent::World, None, None);
    assert_eq!(drawn, Ok(Ok(())));
    assert_eq!(take_method_asks(), 1, "draw");
    assert_eq!(ed::Guest::marker_text(&Core, String::new()), Ok(Ok(())));
    assert_eq!(take_method_asks(), 1, "marker text");
    assert_eq!(ed::Guest::marker_commit(&Core), Ok(()));
    assert_eq!(take_method_asks(), 1, "marker commit");
    assert_eq!(ed::Guest::marker_cancel(&Core), Ok(()));
    assert_eq!(take_method_asks(), 1, "marker cancel");
    // The settings are not read yet, so a set is refused in its answer.
    assert_eq!(st::Guest::set(&Core, 0, 0), Ok(Err(ErrorCode::WouldBlock)));
    assert_eq!(take_method_asks(), 0, "a set refused");
}

/// A method refused changes nothing and asks for no frame, whether its
/// argument is refused or its answer says so.
#[test]
fn a_method_refused_asks_for_no_frame() {
    use crate::abi::{camera as cam, input as inp, scene as sc};
    take_method_asks();
    assert_eq!(cam::Guest::goto(&Core, 181.0, 0.0), Err(Errno::EINVAL));
    assert_eq!(inp::Guest::zoom(&Core, 0.0), Err(Errno::EINVAL));
    let long = "a".repeat(CHAT_TEXT_CHARS as usize + 1);
    assert_eq!(sc::Guest::chat(&Core, long), Ok(Err(ErrorCode::TooLong)));
    assert_eq!(take_method_asks(), 0);
}

/// The device's heading asks for a frame only while the camera follows the
/// device, which is when it turns the camera.
#[test]
fn a_heading_asks_only_while_following() {
    use crate::abi::camera::Tracking;
    use crate::abi::{camera as cam, sensor as sen};
    cam::Guest::track(&Core, Tracking::Unfollow).unwrap();
    take_method_asks();
    assert_eq!(sen::Guest::heading(&Core, Some(10.0)), Ok(()));
    assert_eq!(take_method_asks(), 0, "free");
    cam::Guest::track(&Core, Tracking::Follow).unwrap();
    take_method_asks();
    assert_eq!(sen::Guest::heading(&Core, Some(20.0)), Ok(()));
    assert_eq!(take_method_asks(), 1, "following");
    cam::Guest::track(&Core, Tracking::Unfollow).unwrap();
}

/// A text is held to its kind's cap and refused whole one past it,
/// `too-long`, a chat line and a pin's draft alike, the refusal asking for no
/// frame; a chat line at its cap is the rule's (pxl-spec `text_fits`), as
/// saying it reads the host's clock. A pin's draft holds its text as it is
/// typed, none at first, and keeps the text it had where it is refused.
#[test]
fn a_text_is_held_to_its_kind_cap() {
    use crate::abi::{edit as ed, scene as sc};
    let text = |cnt: u32| "😀".repeat(cnt as usize);
    take_method_asks();
    assert_eq!(
        sc::Guest::chat(&Core, text(CHAT_TEXT_CHARS + 1)),
        Ok(Err(ErrorCode::TooLong))
    );
    assert_eq!(take_method_asks(), 0, "a refused line asks for a frame");

    let [x, y] = flat_at_five();
    write::draft_at(Octant::holding(UNIT_ZOOM, &world::point(x, y, 5.0)));
    assert_eq!(ed::Guest::marker_text(&Core, String::new()), Ok(Ok(())));
    let held = ed::Guest::marker_text(&Core, text(MARKER_TEXT_CHARS));
    assert_eq!(held, Ok(Ok(())));
    take_method_asks();
    assert_eq!(
        ed::Guest::marker_text(&Core, text(MARKER_TEXT_CHARS + 1)),
        Ok(Err(ErrorCode::TooLong))
    );
    assert_eq!(take_method_asks(), 0, "a refused draft asks for a frame");
    assert_eq!(
        write::marker_draft(),
        Some(text(MARKER_TEXT_CHARS).as_str())
    );
    write::cancel_draft();
}
