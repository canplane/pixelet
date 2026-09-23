// pxl-kernel/world/chunk/table.test.rs

use pxl_spec::space::pvf3::ivec;

use super::*;

fn p(in_frustum: bool, centre_distance2: f64) -> Priority {
    Priority {
        holds: false,
        in_frustum,
        hole: false,
        centre_distance2,
        span: 1.0,
        pending_since: -1.0,
    }
}

fn ordered(items: &[(&'static str, Priority)], now: f64) -> Vec<&'static str> {
    let mut v = items.to_vec();
    v.sort_by(|a, b| priority(&a.1, &b.1, now));
    v.into_iter().map(|(n, _)| n).collect()
}

#[test]
fn slot_holders_then_holes_on_screen_then_sharper_then_off_screen() {
    let items = [
        ("far refine", p(true, 900.0)),
        ("near hidden", Priority { hole: true, ..p(false, 1.0) }),
        ("holder", Priority { holds: true, ..p(false, 5000.0) }),
        ("near refine", p(true, 4.0)),
        ("far hole", Priority { hole: true, ..p(true, 2500.0) }),
    ];
    assert_eq!(ordered(&items, 0.0), ["holder", "far hole", "near refine", "far refine", "near hidden"]);
}

#[test]
fn on_screen_a_chunk_is_as_near_as_its_distance_in_its_own_sides() {
    // A coarse chunk sixteen times as wide and eight times as far looks
    // larger than a fine one near, and goes first.
    let coarse = Priority { span: 16.0, ..p(true, 80.0 * 80.0) };
    let fine = p(true, 10.0 * 10.0);
    assert_eq!(ordered(&[("fine", fine), ("coarse", coarse)], 0.0), ["coarse", "fine"]);
    // Off screen the release order is plain distance, farthest first.
    let coarse_off = Priority { span: 16.0, ..p(false, 80.0 * 80.0) };
    assert_eq!(ordered(&[("coarse", coarse_off), ("fine", p(false, 10.0 * 10.0))], 0.0), ["coarse", "fine"]);
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

#[test]
fn a_chunks_children_have_it_for_their_parent_and_its_ancestors_climb_to_the_top() {
    for (x, y) in [(0, 0), (5, -3), (-1, -1), (123, 77)] {
        for level in 1..=PAGE_TOP_LEVEL {
            for (bx, by) in [(0, 0), (64, 448), (448, 64), (256, 192)] {
                let k = ChunkKey {
                    page: LevelPage::new(level, ivec::gather(x, y, 0)),
                    bx,
                    by,
                };
                for c in k.children() {
                    assert_eq!(c.parent(), Some(k), "{k:?} → {c:?}");
                }
                assert_eq!(k.ancestors().count() as u32, PAGE_TOP_LEVEL - level);
            }
        }
    }
}

/// What the mount order has to agree with the build order about: a chunk is
/// as near as its distance in its own sides, so the levels interleave. Left
/// in the key's own order a `ChunkKey` sorts by level, and every chunk of the
/// finest level was mounted before any of the next — which decided what the
/// screen filled in, since a chunk cannot be built before it is mounted.
#[test]
fn mounts_are_ordered_as_the_builds_take_them_and_not_by_level() {
    let chunk = |level: u32, px: i64, bx: u32| ChunkKey {
        page: LevelPage::new(level, ivec::gather(px, 0, 0)),
        bx,
        by: 0,
    };
    // The anchor in the middle of page 0 at every level.
    let (ax, ay) = (0.0, 0.0);
    let near_coarse = chunk(4, 0, 0);
    let far_fine = chunk(0, 8, 0);
    assert!(
        chunk_near(near_coarse, ax, ay) < chunk_near(far_fine, ax, ay),
        "a coarse chunk on the anchor goes before a fine one eight pages out"
    );
    assert!(near_coarse > far_fine, "though the key would have put it last");

    // Within one level it is plain distance.
    assert!(chunk_near(chunk(2, 0, 0), ax, ay) < chunk_near(chunk(2, 1, 0), ax, ay));
    // And a chunk of any level on the anchor's own ground ties with one of
    // another: that is what lets the fill spread outward rather than by
    // octave.
    let mid = |level: u32| chunk_near(chunk(level, 0, 0), ax, ay);
    assert!((mid(0) - mid(4)).abs() < 1e-9, "every level's first chunk is equally near");
}
