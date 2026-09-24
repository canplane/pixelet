// pxl-kernel/world/chunk/mesh_slot.test.rs

use super::*;

/// Chunks a level's drawn ring holds: what one pool's slots are counted in.
const RING_CHUNKS: u32 = RENDER_AXIS * RENDER_AXIS * PAGE_CHUNKS * PAGE_CHUNKS;

#[test]
fn the_two_pools_are_disjoint_and_cover_every_slot() {
    let leaf = pool(0, 1);
    let rest = pool(0, 2);
    assert_eq!(rest, pool(3, 2), "every tier but a leaf's own shares one pool");
    assert_eq!(rest, pool(3, 1), "a level chunk's step 1 is not a leaf's own tier");
    assert!(rest.end <= leaf.start, "disjoint");
    assert_eq!(leaf.end, OCCUPANCY, "and between them, every slot");
    assert_eq!(leaf.len() as u32, RING_CHUNKS, "the leaves' own tier: a drawn ring of them");
    assert_eq!(rest.len() as u32, WINDOW_LEVELS * RING_CHUNKS, "the rest: a ring for every level of the window");
}

#[test]
fn a_slot_lies_inside_its_group_and_no_two_overlap() {
    for p in [pool(0, 1), pool(0, 2)] {
        let mut spans: Vec<(u32, u32, u32)> = p
            .clone()
            .map(|at| {
                let s = slot_at(at);
                assert_eq!(s.index, at, "an index names its own slot");
                (s.group, s.first_box, s.first_box + s.capacity)
            })
            .collect();
        spans.sort_unstable();
        for w in spans.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert!(a.0 != b.0 || a.2 <= b.1, "{a:?} runs into {b:?}");
        }
        for (group, _, end) in spans {
            assert!(end <= group_boxes(group), "group {group} overrun");
        }
    }
}

#[test]
fn a_leaf_s_own_tier_is_the_wider_box_and_every_other_the_narrower() {
    let leaf = slot_at(pool(0, 1).start);
    let rest = slot_at(pool(0, 2).start);
    assert_eq!(leaf.capacity, FINE_BOXES);
    assert_eq!(rest.capacity, COARSE_BOXES);
    assert_eq!(group_box_words(leaf.group), FINE_BOX_WORDS);
    assert_eq!(group_box_words(rest.group), COARSE_BOX_WORDS);
}
