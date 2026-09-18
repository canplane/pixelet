// pxl-kernel/world/chunk/table.test.rs

use super::*;

fn p(in_frustum: bool, lead_distance2: f64) -> Priority {
    Priority {
        holds: false,
        in_frustum,
        empty: false,
        lead_distance2,
        pending_since: -1.0,
    }
}

fn ordered(items: &[(&'static str, Priority)], now: f64) -> Vec<&'static str> {
    let mut v = items.to_vec();
    v.sort_by(|a, b| priority(&a.1, &b.1, now));
    v.into_iter().map(|(n, _)| n).collect()
}

#[test]
fn slot_holders_then_on_screen_then_nearest() {
    let items = [
        ("far refine", p(true, 900.0)),
        ("near hidden", Priority { empty: true, ..p(false, 1.0) }),
        ("holder", Priority { holds: true, ..p(false, 5000.0) }),
        ("near refine", p(true, 4.0)),
        ("far hole", Priority { empty: true, ..p(true, 2500.0) }),
    ];
    assert_eq!(ordered(&items, 0.0), ["holder", "near refine", "far refine", "far hole", "near hidden"]);
}

#[test]
fn a_near_coarse_chunk_goes_before_a_far_empty_one_and_a_hole_wins_a_tie() {
    assert_eq!(
        ordered(&[("far, empty", Priority { empty: true, ..p(true, 1e6) }), ("near, coarse", p(true, 100.0))], 0.0),
        ["near, coarse", "far, empty"]
    );
    assert_eq!(ordered(&[("coarse", p(true, 400.0)), ("empty", Priority { empty: true, ..p(true, 400.0) })], 0.0), ["empty", "coarse"]);
}

#[test]
fn a_wait_overtakes_after_log2_of_how_much_farther_and_never_lifts_onto_the_screen() {
    let first_at = |now: f64| {
        let far = Priority {
            pending_since: 0.0,
            ..p(true, 64.0 * 64.0)
        };
        let near = Priority {
            pending_since: now,
            ..p(true, 8.0 * 8.0)
        };
        ordered(&[("near, fresh", near), ("far, waiting", far)], now)[0]
    };
    assert_eq!(first_at(3.0 * AGE_HALF_MS - 1.0), "near, fresh");
    assert_eq!(first_at(3.0 * AGE_HALF_MS + 1.0), "far, waiting");
    let aged_release = Priority { pending_since: 0.0, ..p(false, 1.0) };
    let fresh_build = Priority { pending_since: 1e6, ..p(true, 1e12) };
    assert_eq!(ordered(&[("release", aged_release), ("build", fresh_build)], 1e6)[0], "build");
    assert_eq!(ordered(&[("near", p(false, 4.0)), ("far", p(false, 900.0))], 0.0), ["far", "near"]);
    let a = Priority { pending_since: 100.0, ..p(true, 9.0) };
    let b = Priority { pending_since: 100.0, ..p(true, 4.0) };
    assert_eq!(ordered(&[("a", a), ("b", b)], 5000.0)[0], "b");
}
