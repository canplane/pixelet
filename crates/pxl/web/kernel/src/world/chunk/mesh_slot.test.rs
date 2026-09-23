// pxl-kernel/world/chunk/mesh_slot.test.rs

use super::*;

#[test]
fn a_slot_is_the_chunks_residue_and_slots_never_overlap() {
    let a = AXIS as i64;
    // A ring apart, one slot; one chunk apart, two.
    assert_eq!(slot_of(3, 5, 7, 2), slot_of(3, 5 + a, 7 - a, 2));
    assert_ne!(slot_of(3, 5, 7, 2), slot_of(3, 6, 7, 2));
    // A window apart, one group; the leaves' own tier, a group of its own.
    assert_eq!(slot_of(1, 0, 0, 2), slot_of(1 + WINDOW_LEVELS, 0, 0, 2));
    assert_ne!(slot_of(0, 0, 0, 1).group, slot_of(0, 0, 0, 2).group);
    assert_ne!(slot_of(0, 0, 0, 1).index, slot_of(0, 0, 0, 2).index);
    // Every slot of a group lies inside it and apart from the others.
    for group in 0..GROUPS {
        let mut spans: Vec<(u32, u32)> = Vec::new();
        for step in [1, 2] {
            for y in 0..a {
                for x in 0..a {
                    let s = slot_of(if group == GROUPS - 1 { 0 } else { group }, x, y, step);
                    if s.group != group {
                        continue;
                    }
                    spans.push((s.first_box, s.first_box + s.capacity));
                }
            }
        }
        spans.sort_unstable();
        spans.dedup();
        assert!(spans.windows(2).all(|w| w[0].1 <= w[1].0), "group {group}");
        assert!(spans.last().unwrap().1 <= group_boxes(group), "group {group}");
    }
}
