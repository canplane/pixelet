// pxl-kernel/io/body.test.rs

use super::*;

/// `.fixed.bodies` as a test has it: memory of its size, placed once and
/// never freed, as boot places the range, with every slot back.
fn placed() -> &'static mut Slots {
    let s = held();
    if s.at.is_none() {
        let room: &'static mut [u8] = Box::leak(vec![0u8; BODIES_BYTES as usize].into_boxed_slice());
        s.at = Some(xpute_guest::mem::base::off_of(room.as_ptr()));
    }
    s.out = 0;
    s
}

#[test]
fn terrain_and_earth_bodies_take_the_slots_until_every_one_is_out_and_then_wait_for_one_given_back() {
    let _g = crate::serial();
    placed();
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..SLOTS).map(|k| take(Some(if k % 2 == 0 { Stream::terrain } else { Stream::earth }), longest).unwrap()).collect();
    let mut at: Vec<usize> = out.iter().map(|b| b.0).collect();
    at.sort();
    assert!(at.windows(2).all(|w| w[1] - w[0] >= SLOT_BYTES as usize), "no two slots overlap");

    let refused = take(Some(Stream::terrain), longest).unwrap_err();
    assert_eq!(refused, Refused::bodies(), "every slot out: refused by the slots");
    assert!(!refused.over(), "and waits");

    for (k, &(at, n)) in out.iter().enumerate() {
        // SAFETY: `n` bytes of a slot just taken.
        unsafe { core::ptr::write_bytes(xpute_guest::mem::base::ptr_at::<u8>(at), k as u8, n as usize) };
    }
    let freed = out.remove(1);
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(freed) };
    assert!(refused.over(), "a slot given back ends the wait");
    let again = take(Some(Stream::earth), longest).unwrap();
    assert_eq!(again.0, freed.0, "and the slot given back is the one taken");
    for (k, &(at, n)) in out.iter().enumerate().skip(1) {
        // SAFETY: still out, `n` bytes long.
        let bytes = unsafe { core::slice::from_raw_parts(xpute_guest::mem::base::ptr_at::<u8>(at), n as usize) };
        assert!(bytes.iter().all(|&b| b == (k + 1) as u8), "a body was written over by its neighbors");
    }

    out.push(again);
    for block in out {
        unsafe { give(block) };
    }
    assert_eq!(held().out, 0, "every slot back");
}

#[test]
fn every_other_body_is_a_heap_block_of_its_own() {
    let _g = crate::serial();
    placed();
    for stream in [Some(Stream::place), Some(Stream::division), None] {
        let block = take(stream, 4096).unwrap();
        assert_eq!(held().out, 0, "{stream:?} takes no slot");
        let released = releases();
        // SAFETY: from `take`, given back once.
        unsafe { give(block) };
        assert_eq!(releases(), released, "nor counts one given back");
    }
}

#[test]
fn the_peak_is_the_most_slots_out_since_it_was_last_taken() {
    let _g = crate::serial();
    placed();
    take_peak();
    let a = take(Some(Stream::terrain), 1).unwrap();
    let b = take(Some(Stream::earth), 1).unwrap();
    // SAFETY: from `take`, given back once.
    unsafe { give(a) };
    assert_eq!(take_peak(), 2, "two were out at once");
    assert_eq!(take_peak(), 1, "and from then, the one still out");
    unsafe { give(b) };
}

#[test]
fn a_slot_costs_less_than_the_buddy_block_it_replaces() {
    for stream in SLOTTED {
        assert!(SLOT_BYTES >= longest(stream) && SLOT_BYTES < longest(stream).next_power_of_two());
    }
    assert_eq!(BODIES_BYTES, (MAX_CREDITS + KEPT_BODIES) * SLOT_BYTES);
}
