// pxl-kernel/world/chunk/table.test.rs

use pxl_spec::space::pvf3::ivec;

use super::*;

fn p(in_frustum: bool, centre_distance2: f64) -> Priority {
    Priority {
        holds: false,
        in_frustum,
        hole: false,
        restale: false,
        centre_distance2,
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
                let k = ChunkKey {
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
    // Within one level it is plain distance, as it is across them.
    assert!(chunk_near(chunk(2, 0, 0), ax, ay) < chunk_near(chunk(2, 1, 0), ax, ay));
    // A coarse chunk is wider, so its centre is further from the anchor even
    // when it starts there: the middle of the screen is drawn by the finest
    // level, and that is what goes first.
    assert!(chunk_near(chunk(0, 0, 0), ax, ay) < chunk_near(chunk(4, 0, 0), ax, ay));
}

/// Where the selection starts when a page of the top ring has not arrived.
/// The adversarial cases of a zoom out, which is when a top ring is new.
mod selection_roots {
    use super::*;

    fn page(level: u32, x: i64, y: i64) -> LevelPage {
        LevelPage::new(level, ivec::gather(x, y, 0))
    }

    fn held(of: &[LevelPage]) -> BTreeSet<LevelPage> {
        of.iter().copied().collect()
    }

    fn roots_of(pages: &BTreeSet<LevelPage>, top: LevelPage, base: u32) -> Vec<LevelPage> {
        let mut out = Vec::new();
        roots(pages, top, base, &mut out);
        out.sort_unstable();
        out
    }

    #[test]
    fn a_resident_page_is_the_root_and_its_children_are_left_to_the_split() {
        let top = page(4, 3, 5);
        let mut pages = held(&[top]);
        pages.extend(top.children());
        // Both resident: the page itself roots, and whether the finer ones
        // are drawn is the ladder's question (`splits`), not this one.
        assert_eq!(roots_of(&pages, top, 0), [top]);
    }

    #[test]
    fn a_missing_page_is_stood_in_for_by_every_child_that_is_resident() {
        let top = page(4, 3, 5);
        let kids = top.children();
        // All four: the whole of its ground is drawn, a level finer.
        let all = held(&kids);
        assert_eq!(roots_of(&all, top, 0).len(), 4);
        // Two of four, which is what a held ring actually leaves
        // (terrain/level.test.rs): half the ground rather than none of it.
        let some = held(&kids[..2]);
        assert_eq!(roots_of(&some, top, 0), {
            let mut v = kids[..2].to_vec();
            v.sort_unstable();
            v
        });
    }

    #[test]
    fn it_goes_one_level_and_no_further_so_a_seam_is_never_more_than_one_apart() {
        let top = page(4, 0, 0);
        let deep = top.children()[0].children()[0];
        assert!(roots_of(&held(&[deep]), top, 0).is_empty(), "a grandchild is not stood in for");
        assert!(roots_of(&held(&top.children()), top, top.level).is_empty(), "not past the base");
    }

    #[test]
    fn nothing_resident_under_it_is_nothing_drawn_and_not_a_guess() {
        assert!(roots_of(&held(&[]), page(4, 3, 5), 0).is_empty());
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

#[test]
fn presentation_snaps_to_the_committed_tier_and_waits_for_a_drawn_surface() {
    let mut s = Scheduler::new();
    s.add(1, 0, 0, -64.0, 128.0, 0, 0, 0.0, true);
    assert_eq!(s.surface_anchor(-60.1, 130.9), None);
    let mesh = slot_at(pool(0, 1).start);
    s.meshes.surface[mesh.index] = vec![11; 64 * 64].into_boxed_slice();
    let chunk = &mut s.slots[0];
    chunk.layout_primary = 1;
    chunk.layout_ready = true;
    chunk.set(IN_FRUSTUM, true);
    chunk.tiers.push(Tier {
        step: 1,
        generation: 0,
        bounds: None,
        slot: mesh,
        box_cnt: 1,
        heights: Heights { floor: 0.0, step: 1.0 },
    });
    assert_eq!(s.surface_anchor(-60.1, 130.9), Some([-60.5, 130.5, 10.0]));
    let coarse = slot_at(pool(0, 2).start);
    s.meshes.surface[coarse.index] = vec![15; 32 * 32].into_boxed_slice();
    s.slots[0].tiers.push(Tier {
        step: 2,
        generation: 0,
        bounds: None,
        slot: coarse,
        box_cnt: 1,
        heights: Heights { floor: 0.0, step: 1.0 },
    });
    s.slots[0].layout_secondary = 2;
    s.slots[0].told = Some((0.4, 0.6, true));
    assert_eq!(s.surface_anchor(-60.1, 130.9), Some([-61.0, 131.0, 14.0]));
    let canonical = [-60.1, 130.9, -100.0, -60.1, 130.9, -100.0];
    let envelope = s.surface_envelopes()[0];
    let anchor = s.surface_anchor(-60.1, 130.9).unwrap();
    assert!(envelope[0] <= anchor[0] && envelope[2] >= anchor[0]);
    assert!(envelope[1] <= anchor[1] && envelope[3] >= anchor[1]);
    assert!(envelope[4] <= anchor[2] && envelope[5] >= anchor[2]);
    assert_eq!(canonical[2], -100.0);
    s.slots[0].set(COVERED, true);
    assert_eq!(s.surface_anchor(-60.1, 130.9), None);
}
