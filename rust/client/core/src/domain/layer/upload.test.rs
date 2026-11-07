// pxl-core/domain/layer/upload.test.rs

use super::*;
use xpute_core::mem::heap::alloc;

fn block() -> Block {
    alloc(64).unwrap()
}

/// Whether slot `slot`'s packet is still to be sent.
fn waits(k: &Uploads, slot: usize) -> bool {
    matches!(k.slots[slot], Upload::Waiting(_))
}

#[test]
fn keeps_at_most() {
    let mut k = Uploads::new(4);
    for slot in 0..3 {
        k.keep(Some(slot), block(), 2);
    }
    assert_eq!(k.waiting, 2);
    assert!(
        waits(&k, 0) && waits(&k, 1) && !waits(&k, 2),
        "the third is past the bound"
    );
    assert_eq!(k.sent_on(2), None);
    k.keep(Some(0), block(), 2);
    assert_eq!(k.waiting, 2, "a slot's packet replaced is still one");
    k.send(7, &mut |slot, _| {
        if slot == 0 {
            Uptake::Taken
        } else {
            Uptake::Full
        }
    });
    assert_eq!(
        (k.waiting, k.sent_on(0)),
        (1, Some(7)),
        "taken: freed, and sent"
    );
    k.keep(Some(3), block(), 2);
    assert_eq!(k.waiting, 2, "room again once one is taken");
    k.send(7, &mut |_, _| Uptake::Taken);
    assert_eq!(k.waiting, 0);
}

#[test]
fn full_stream_offers_no_more() {
    let mut k = Uploads::new(4);
    for slot in 0..4 {
        k.keep(Some(slot), block(), 4);
    }
    let mut offered = Vec::new();
    k.send(3, &mut |slot, _| {
        offered.push(slot);
        Uptake::Full
    });
    assert_eq!(offered, [0], "nothing offered past the stream's end");
    assert_eq!(k.waiting, 4, "each kept for a later turn");
}
