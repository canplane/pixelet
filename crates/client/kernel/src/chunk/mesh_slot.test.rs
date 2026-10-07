// pxl-kernel/chunk/mesh_slot.test.rs

use super::*;

#[test]
fn a_slot_lies_inside_its_group_and_no_two_overlap() {
    for p in [Pool::Fine, Pool::Coarse] {
        let mut spans: Vec<(u32, u32, u32)> = p
            .range()
            .map(|at| {
                assert_eq!(Pool::at(at), p, "a slot is in the pool that holds it");
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
