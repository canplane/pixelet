// xpute-guest/ipc/ring.test.rs

use super::*;
use crate::ipc::frame;
use xpute_kit::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

fn ring(capacity: u32, slot: u32) -> (Ring, Box<[u8]>) {
    let slot_base = 8 + ring_bytes(capacity);
    let mut buf = vec![0u8; (slot_base + slots_bytes(capacity, slot)) as usize].into_boxed_slice();
    // SAFETY: the buffer outlives the ring — both are returned together and
    // the caller holds them for the test.
    let r = unsafe { Ring::init(buf.as_mut_ptr(), 8, capacity, slot_base, slot) };
    (r, buf)
}

#[test]
fn ring_messages_come_out_in_order_across_the_wrap_and_a_full_ring_refuses() {
    let (r, _buf) = ring(4, 64);
    let ack = frame::ACKREQ;
    for round in 0..3u32 {
        for k in 0..4 {
            assert_eq!(r.push(round * 4 + k, 7, ack, None, -((round * 4 + k) as i32)), Errno::OK);
        }
        assert_eq!(r.push(99, 0, 0, None, 0), Errno::EAGAIN, "a full ring refuses");
        for k in 0..4 {
            let at = r.peek();
            assert!(at >= 0);
            let at = at as u32;
            assert_eq!((r.tag(at), r.cmd(at), r.flags(at), r.result(at)), (round * 4 + k, 7, ack, -((round * 4 + k) as i32)));
            assert!(r.packet(at).unwrap().is_none());
            r.advance();
        }
        assert_eq!(r.peek(), -1);
    }
    assert_eq!([r.head(), r.tail()], [12, 12], "positions only grow");
}

#[test]
fn ring_one_payload_a_slot_so_the_payloads_run_out_with_the_slots_and_never_before_them() {
    let (r, _buf) = ring(8, 64);
    let pkt = |n: u32| {
        let mut t: TreeView<'_> = TreeView::new();
        t.branch(Some(&mut |b| {
            b.u32(Some(n)).u64(Some((n as u64) << 40));
        }));
        encoder().encode(&t, TreeEncoderOptions::default()).unwrap()
    };
    assert_eq!(pkt(1).len(), 56);

    for n in 1..=8u32 {
        assert_eq!(r.push(n, 0x0101, 0, Some(&pkt(n)), 0), Errno::OK, "every slot has a payload of its own");
    }
    assert_eq!(r.push(9, 0x0101, 0, Some(&pkt(9)), 0), Errno::EAGAIN, "and the ring fills before the payloads do");
    assert_eq!(r.len(), 8);

    for n in 1..=8u32 {
        let at = r.peek() as u32;
        let root = TreeReader::new(r.packet(at).unwrap().unwrap()).unwrap().read_branch().unwrap();
        assert_eq!(
            (r.tag(at), root.at(0).unwrap().get_number().unwrap() as u32, root.at(1).unwrap().get_u64().unwrap()),
            (n, n, (n as u64) << 40)
        );
        r.advance();
    }
    assert_eq!(r.push(9, 0x0101, 0, Some(&pkt(9)), 0), Errno::OK, "a slot freed is a payload freed");

    assert_eq!(r.push(10, 0x0101, 0, Some(&[0u8; 72]), 0), Errno::EMSGSIZE);
    let at = r.entry_at(r.tail());
    r.push(11, 0, 0, None, 0);
    r.words()[(at + frame::PACKET) as usize] = 4;
    assert!(r.packet(at).is_err(), "outside a slot");
}
