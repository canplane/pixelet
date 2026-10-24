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
    k.send(7, &mut |record, _| if record == 0 { Offered::Taken } else { Offered::Kept });
    assert_eq!((k.count(), k.sent_on(0)), (1, Some(7)), "taken whole: freed, and sent");
    k.keep(Some(3), block(), 2);
    assert_eq!(k.count(), 2, "room again once one is taken");
    k.send(7, &mut |_, _| Offered::Taken);
    assert_eq!(k.count(), 0);
}
