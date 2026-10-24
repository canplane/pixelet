// pxl-kernel/layer/kept.test.rs

use super::*;
use xpute_guest::mem::heap::alloc;

fn block() -> Block {
    alloc(64).unwrap()
}

#[test]
fn no_more_than_most_are_kept_and_one_past_it_is_freed_with_its_frame_holding_nothing() {
    let mut k = Kept::new(4);
    for record in 0..3 {
        k.keep(Some(record), block(), 2);
    }
    assert_eq!(k.count(), 2);
    assert!(k.waiting(0) && k.waiting(1) && !k.waiting(2), "the third is past the bound");
    assert_eq!(k.sent_on(2), None);
    k.keep(Some(0), block(), 2);
    assert_eq!(k.count(), 2, "a frame's packet replaced is still one");
    k.send(7, &mut |record, _| if record == 0 { Offered::Taken } else { Offered::Kept }, &|_| true);
    assert_eq!((k.count(), k.sent_on(0)), (1, Some(7)), "taken whole: freed, and sent");
    k.keep(Some(3), block(), 2);
    assert_eq!(k.count(), 2, "room again once one is taken");
    k.send(7, &mut |_, _| Offered::Taken, &|_| true);
    assert_eq!(k.count(), 0);
}

#[test]
fn past_a_full_stream_the_packets_nothing_wants_are_freed_unsent() {
    let mut k = Kept::new(4);
    for record in 0..4 {
        k.keep(Some(record), block(), 4);
    }
    let mut offered = Vec::new();
    k.send(
        3,
        &mut |record, _| {
            offered.push(record);
            Offered::Full
        },
        &|record| record == 2,
    );
    assert_eq!(offered, [0], "nothing offered past the stream's end");
    assert!(k.waiting(0) && k.waiting(2), "the one offered and the one still wanted are kept");
    assert!(!k.waiting(1) && !k.waiting(3) && k.sent_on(1).is_none(), "the rest freed, and not sent");
    assert_eq!(k.count(), 2);
}

#[test]
fn a_frame_forgotten_holds_nothing_and_counts_none() {
    let mut k = Kept::new(2);
    k.keep(Some(0), block(), 2);
    k.forget(0);
    k.forget(1);
    assert!(!k.waiting(0) && k.count() == 0);
}
