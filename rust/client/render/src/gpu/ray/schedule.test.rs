// pxl-render/gpu/ray/schedule.test.rs

use super::*;

/// What a frame reads where nothing moved since the last.
fn still() -> Reads {
    Reads {
        same: true,
        jitter: [0; 2],
        partial: false,
        shadows: true,
        light_made: true,
        content_same: true,
        world_columns: false,
        corner: [0; 2],
        range: [0; 2],
        receivers_same: true,
    }
}

/// A schedule whose rays and map hold what `still` reads.
fn stood() -> Schedule {
    let mut s = Schedule::new();
    for _ in 0..3 {
        s.plan(still());
    }
    s
}

#[test]
fn nothing_moved_casts_nothing() {
    let mut s = stood();
    let plan = s.plan(still());
    assert_eq!(plan.kind, FrameKind::standing);
    assert!(plan.light.is_none());
}

#[test]
fn a_moved_view_casts_the_rays() {
    let mut s = stood();
    let plan = s.plan(Reads {
        same: false,
        ..still()
    });
    assert_eq!(plan.kind, FrameKind::cast);
}

#[test]
fn the_jitter_casts_the_pixels_alone() {
    let mut s = stood();
    let plan = s.plan(Reads {
        jitter: [1, 0],
        ..still()
    });
    assert_eq!(plan.kind, FrameKind::px);
}

#[test]
fn a_dropped_frame_casts_again() {
    let mut s = stood();
    s.dropped();
    assert_eq!(s.plan(still()).kind, FrameKind::cast);
}

#[test]
fn the_light_casts_where_it_moved() {
    for moved in [
        Reads {
            content_same: false,
            ..still()
        },
        Reads {
            corner: [8, 0],
            ..still()
        },
        Reads {
            receivers_same: false,
            ..still()
        },
        Reads {
            world_columns: true,
            ..still()
        },
    ] {
        let mut s = stood();
        assert!(s.plan(moved).light.is_some());
    }
}

#[test]
fn no_shadows_no_light() {
    let mut s = stood();
    let plan = s.plan(Reads {
        shadows: false,
        receivers_same: false,
        ..still()
    });
    assert!(plan.light.is_none());
}

/// A view that stops stands again by the second frame after it moved, and stays standing.
#[test]
fn a_stopped_view_rests() {
    let mut s = stood();
    s.plan(Reads {
        same: false,
        ..still()
    });
    s.plan(still());
    assert_eq!(s.plan(still()).kind, FrameKind::standing);
    assert_eq!(s.plan(still()).kind, FrameKind::standing);
}
