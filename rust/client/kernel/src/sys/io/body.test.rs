// pxl-kernel/sys/io/body.test.rs

use super::*;

fn runs() -> usize {
    held().runs.len()
}

#[test]
fn bodies_share_run() {
    let _g = crate::sys::serial();
    let before = runs();
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..SLOTS)
        .map(|k| {
            take(
                Some(if k % 2 == 0 {
                    Stream::terrain
                } else {
                    Stream::earth
                }),
                longest,
            )
            .unwrap()
        })
        .collect();
    assert_eq!(runs(), before + 1, "{SLOTS} bodies fill one run");
    let mut at: Vec<usize> = out.iter().map(|b| b.0).collect();
    at.sort();
    assert!(
        at.windows(2).all(|w| w[1] - w[0] >= SLOT_BYTES as usize),
        "no two slots overlap"
    );
    out.push(take(Some(Stream::terrain), longest).unwrap());
    assert_eq!(runs(), before + 2, "and the next borrows another");

    for (k, &(at, n)) in out.iter().enumerate() {
        // SAFETY: `n` bytes of a slot just taken.
        unsafe { core::ptr::write_bytes(xpute_guest::mem::at::<u8>(at), k as u8, n as usize) };
    }
    let released = releases();
    let freed = out.remove(1);
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(freed) };
    assert_eq!(
        releases(),
        released.wrapping_add(1),
        "a slot given back is a release"
    );
    let again = take(Some(Stream::earth), longest).unwrap();
    assert_eq!(
        again.0, freed.0,
        "a slot given back is taken again before a run is borrowed"
    );
    assert_eq!(runs(), before + 2);
    for (k, &(at, n)) in out.iter().enumerate().skip(1) {
        // SAFETY: still out, `n` bytes long.
        let bytes =
            unsafe { core::slice::from_raw_parts(xpute_guest::mem::at::<u8>(at), n as usize) };
        assert!(
            bytes.iter().all(|&b| b == (k + 1) as u8),
            "a body was written over by its neighbors"
        );
    }

    out.push(again);
    for block in out {
        unsafe { give(block) };
    }
    assert_eq!(runs(), before, "a run is given back once its last slot is");
}

#[test]
fn other_bodies_own_block() {
    let _g = crate::sys::serial();
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
fn slots_cap_and_wake() {
    let _g = crate::sys::serial();
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..BODY_SLOTS)
        .map(|_| take(Some(Stream::terrain), longest).unwrap())
        .collect();
    let refused = take(Some(Stream::earth), longest).unwrap_err();
    assert!(!refused.over(), "every slot out: the body waits");
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(out.pop().unwrap()) };
    assert!(refused.over(), "until one is given back");
    for block in out {
        unsafe { give(block) };
    }
}

#[test]
fn slot_peak() {
    let _g = crate::sys::serial();
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
fn slot_cheaper_than_block() {
    assert_eq!(SLOTS, 7, "at today's packets, as sys/io/body.rs says");
    for stream in SLOTTED {
        assert!(
            SLOT_BYTES >= longest(stream)
                && (BODY_RUN_BYTES / SLOTS) < longest(stream).next_power_of_two()
        );
    }
}

#[test]
fn slot_refusal_waits_slots() {
    use crate::sys::boot::set_heap_stat;
    use core::sync::atomic::{AtomicU32, Ordering};
    use xpute_guest::mem::heap::HeapStat;
    static RELEASES: AtomicU32 = AtomicU32::new(0);
    let _g = crate::sys::serial();
    set_heap_stat(|| HeapStat {
        releases: RELEASES.load(Ordering::Relaxed),
        ..HeapStat::default()
    });
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..BODY_SLOTS)
        .map(|_| take(Some(Stream::terrain), longest).unwrap())
        .collect();
    let refused = take(Some(Stream::terrain), longest).unwrap_err();
    RELEASES.fetch_add(1, Ordering::Relaxed);
    assert!(!refused.over(), "the heap's pages given back give no slot");
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(out.pop().unwrap()) };
    assert!(refused.over());
    for block in out {
        unsafe { give(block) };
    }
    set_heap_stat(HeapStat::default);
}

#[test]
fn run_refusal_waits_heap() {
    let _g = crate::sys::serial();
    let refused = crate::sys::io::room::Refused::run();
    assert!(!refused.over());
    let block = take(Some(Stream::terrain), 1).unwrap();
    // SAFETY: from `take`, given back once.
    unsafe { give(block) };
    assert!(refused.over(), "a slot given back in a run held serves it");
}

#[test]
fn slot_return_asks_turn() {
    let _g = crate::sys::serial();
    let longest = longest(Stream::terrain);
    let mut out: Vec<Block> = (0..BODY_SLOTS)
        .map(|_| take(Some(Stream::terrain), longest).unwrap())
        .collect();
    assert!(take(Some(Stream::terrain), longest).is_err());
    assert!(held().waited, "refused for want of a slot");
    // SAFETY: from `take`, given back once, here and below.
    unsafe { give(out.pop().unwrap()) };
    assert!(!held().waited, "the slot given back asked the turn");
    for block in out {
        unsafe { give(block) };
    }
}
