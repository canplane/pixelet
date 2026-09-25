// xpute-core/collection/arena.test.rs
// (no pair: arena.ts has no test file)

use super::*;

#[derive(Default, PartialEq, Debug)]
struct Slot {
    /// What a slot owns and keeps across a rewind, which is the whole point:
    /// a `Vec`'s buffer is still allocated when the slot is handed out again.
    kept: Vec<u32>,
    mark: u32,
}

fn arena(init_cap: u32, max_cap: u32) -> Arena<Slot> {
    Arena::new(ArenaOptions { init_cap, max_cap }).unwrap()
}

/// The reason a pool is not a `Vec`: what a slot owns survives the rewind, so
/// a pass that fills the same slots again pays no allocation for them.
#[test]
fn a_rewind_drops_nothing_so_a_slot_s_own_buffer_is_there_the_next_time() {
    let mut a = arena(4, 1 << 10);
    for i in 0..4 {
        let slot = a.alloc().unwrap();
        slot.kept.extend([i, i + 1, i + 2]);
        slot.mark = i;
    }
    let caps: Vec<usize> = (0..4).map(|i| a.get(i).kept.capacity()).collect();
    assert!(caps.iter().all(|c| *c >= 3));

    a.truncate(0);
    assert_eq!(a.len(), 0);

    for i in 0..4 {
        let slot = a.alloc().unwrap();
        assert_eq!(slot.mark, i, "a rewound slot was rebuilt rather than kept");
        assert_eq!(slot.kept, vec![i, i + 1, i + 2], "a rewound slot lost what it owned");
    }
    let after: Vec<usize> = (0..4).map(|i| a.get(i).kept.capacity()).collect();
    assert_eq!(after, caps, "a rewind gave a slot's buffer back");
}

/// Growth doubles and carries what was there; `reserve` does it in one step so
/// that the allocs after it do not.
#[test]
fn growth_doubles_past_the_cap_and_carries_what_was_written() {
    let mut a = arena(2, 1 << 10);
    assert_eq!(a.cap(), 2);
    for i in 0..5 {
        a.alloc().unwrap().mark = i;
    }
    assert_eq!(a.cap(), 8, "the cap did not double to hold five");
    for i in 0..5 {
        assert_eq!(a.get(i).mark, i, "growth lost what a slot held");
    }

    a.reserve(100).unwrap();
    let cap = a.cap();
    assert!(cap >= 105);
    for _ in 0..100 {
        a.alloc().unwrap();
    }
    assert_eq!(a.cap(), cap, "an alloc after reserve grew again");
}

/// `max_cap` is a refusal and not a clamp: the arena is left as it was, and
/// what was in it is still readable.
#[test]
fn growth_past_max_cap_is_refused_and_leaves_the_arena_as_it_was() {
    let mut a = arena(2, 4);
    for i in 0..4 {
        a.alloc().unwrap().mark = i + 10;
    }
    assert_eq!(a.cap(), 4);

    let refused = a.alloc();
    assert!(refused.is_err(), "the arena grew past max_cap");
    assert_eq!(a.len(), 4, "a refused alloc moved the cursor");
    assert_eq!(a.cap(), 4);
    for i in 0..4 {
        assert_eq!(a.get(i).mark, i + 10, "a refusal wrote over a live slot");
    }
    assert!(a.reserve(1).is_err());
}

/// The scope is the rewind written as a lifetime: what a pass took is given
/// back when it ends, whichever way it ends.
#[test]
fn a_scope_rewinds_to_where_it_began() {
    let mut a = arena(8, 1 << 10);
    a.alloc().unwrap().mark = 1;
    assert_eq!(a.len(), 1);
    {
        let mut pass = a.scope();
        for _ in 0..5 {
            pass.alloc().unwrap();
        }
        assert_eq!(pass.len(), 6);
    }
    assert_eq!(a.len(), 1, "the scope did not rewind to its mark");
    assert_eq!(a.get(0).mark, 1, "the rewind reached past the mark");
}
