// pxl-kernel/world/chunk/table.test.rs

use pxl_spec::space::pvf3::ivec;

use super::*;

fn p(in_frustum: bool, distance2: f64) -> Priority {
    Priority {
        holds: false,
        in_frustum,
        hole: false,
        restale: false,
        distance2,
        pending_since: -1.0,
    }
}

fn ordered(items: &[(&'static str, Priority)], now: f64) -> Vec<&'static str> {
    let mut v = items.to_vec();
    v.sort_by(|a, b| priority(&a.1, &b.1, now));
    v.into_iter().map(|(n, _)| n).collect()
}

#[test]
fn slot_holders_then_holes_then_a_generation_behind_then_sharper_then_off_screen() {
    let items = [
        ("far refine", p(true, 900.0)),
        ("near hidden", Priority { hole: true, ..p(false, 1.0) }),
        ("holder", Priority { holds: true, ..p(false, 5000.0) }),
        ("near refine", p(true, 4.0)),
        ("far hole", Priority { hole: true, ..p(true, 2500.0) }),
        // Its page has moved past what it draws: worse than coarse, better
        // than nothing, and far does not save it from the class above.
        ("far restale", Priority { restale: true, ..p(true, 3600.0) }),
    ];
    assert_eq!(ordered(&items, 0.0), ["holder", "far hole", "far restale", "near refine", "far refine", "near hidden"]);
}

/// What the class used to lose by being a list of its own: a page arriving
/// for the horizon sent its chunks ahead of the ones under the camera, in
/// the order the network answered. Inside the class the distance decides.
#[test]
fn a_generation_behind_is_still_ordered_nearest_first() {
    let items = [("far", Priority { restale: true, ..p(true, 9000.0) }), ("near", Priority { restale: true, ..p(true, 9.0) })];
    assert_eq!(ordered(&items, 0.0), ["near", "far"]);
}

/// On screen the nearest the view centre goes first, whatever its level.
///
/// It used to be the distance in the chunk's own sides — "the biggest on the
/// screen first" — and the ladder makes that the same for every level, so a
/// coarse chunk at the horizon went with a fine one under the camera and the
/// screen filled everywhere at once. Everywhere at once means the middle
/// waits for all of it.
#[test]
fn on_screen_the_nearest_the_middle_goes_first_whatever_its_level() {
    let far_coarse = p(true, 80.0 * 80.0);
    let near_fine = p(true, 10.0 * 10.0);
    assert_eq!(ordered(&[("far", far_coarse), ("near", near_fine)], 0.0), ["near", "far"]);
    // Off screen the release order is the same distance, farthest first: a
    // release takes memory from what is least likely to be wanted again.
    assert_eq!(ordered(&[("far", p(false, 80.0 * 80.0)), ("near", p(false, 10.0 * 10.0))], 0.0), ["far", "near"]);
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
        for level in 1..=TOP_LEVEL {
            for (bx, by) in [(0, 0), (64, 448), (448, 64), (256, 192)] {
                let k = Chunk {
                    page: LevelPage::new(level, ivec::gather(x, y, 0)),
                    bx,
                    by,
                };
                for c in k.children() {
                    assert_eq!(c.parent(), Some(k), "{k:?} → {c:?}");
                }
                assert_eq!(k.ancestors().count() as u32, TOP_LEVEL - level);
            }
        }
    }
}

#[test]
fn surface_centres_use_drawn_box_tops_and_leave_empty_centres_empty() {
    let heights = Heights { floor: -20.0, step: 0.25 };
    // Two fine boxes occupying the first two cells, at different top heights.
    let words = [0, 256 | (256 << 16), 20 << 16, 0, 0, 0, 256, 256 | (256 << 16), 4 | (24 << 16), 0, 0, 0];
    let tops = surface_centres(&words, 6, 1, 0, heights);
    assert_eq!(&tops[..3], &[21, 29, 0]);
    assert_eq!(heights.floor + (tops[1] - 1) as f64 * heights.step, -13.0);
    // A coarse box shrunk about its cell's centre still grounds that cell.
    let coarse = [0, 0, 40 << 16, 2 | (2 << 8) | (3 << 16) | (3 << 24)];
    let tops = surface_centres(&coarse, 4, 2, 3, heights);
    assert_eq!(tops[0], 41);
    assert_eq!(tops[1], 0);
    assert_eq!(tops.len(), 32 * 32);
}
