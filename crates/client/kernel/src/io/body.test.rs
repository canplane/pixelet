// pxl-kernel/io/body.test.rs

use super::*;

fn runs() -> usize {
    held().runs.len()
}

#[test]
fn terrain_and_earth_bodies_share_a_run_until_it_is_full_and_it_goes_once_the_last_is_back() {
    let _g = crate::serial();
    let before = runs();
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..SLOTS).map(|k| take(Some(if k % 2 == 0 { Stream::terrain } else { Stream::earth }), longest).unwrap()).collect();
    assert_eq!(runs(), before + 1, "{SLOTS} bodies fill one run");
    let mut at: Vec<usize> = out.iter().map(|b| b.0).collect();
    at.sort();
    assert!(at.windows(2).all(|w| w[1] - w[0] >= SLOT_BYTES as usize), "no two slots overlap");
    out.push(take(Some(Stream::terrain), longest).unwrap());
    assert_eq!(runs(), before + 2, "and the next borrows another");

    for (k, &(at, n)) in out.iter().enumerate() {
        // SAFETY: `n` bytes of a slot just taken.
        unsafe { core::ptr::write_bytes(xpute_guest::mem::base::ptr_at::<u8>(at), k as u8, n as usize) };
    }
    let released = releases();
    let freed = out.remove(1);
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(freed) };
    assert_eq!(releases(), released.wrapping_add(1), "a slot given back is a release");
    let again = take(Some(Stream::earth), longest).unwrap();
    assert_eq!(again.0, freed.0, "a slot given back is taken again before a run is borrowed");
    assert_eq!(runs(), before + 2);
    for (k, &(at, n)) in out.iter().enumerate().skip(1) {
        // SAFETY: still out, `n` bytes long.
        let bytes = unsafe { core::slice::from_raw_parts(xpute_guest::mem::base::ptr_at::<u8>(at), n as usize) };
        assert!(bytes.iter().all(|&b| b == (k + 1) as u8), "a body was written over by its neighbors");
    }

    out.push(again);
    for block in out {
        unsafe { give(block) };
    }
    assert_eq!(runs(), before, "a run is given back once its last slot is");
}

#[test]
fn every_other_body_is_a_heap_block_of_its_own() {
    let _g = crate::serial();
    let before = runs();
    for stream in [Some(Stream::place), Some(Stream::division), None] {
        let block = take(stream, 4096).unwrap();
        assert_eq!(runs(), before, "{stream:?} takes no slot");
        let released = releases();
        // SAFETY: from `take`, given back once.
        unsafe { give(block) };
        assert_eq!(releases(), released, "nor counts one given back");
    }
}

#[test]
fn a_slot_costs_less_than_the_buddy_block_it_replaces() {
    assert_eq!(SLOTS, 7, "at today's packets, as io/body.rs says");
    for stream in SLOTTED {
        assert!(SLOT_BYTES >= longest(stream) && (RUN_BYTES / SLOTS) < longest(stream).next_power_of_two());
    }
}
