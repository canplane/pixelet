// xpute-runtime/ipc/ring.test.rs

use super::*;
use crate::ipc::frame::FrameFlag;
use xpute_core::wire::xtp::{encoder, NodeView, TreeEncoderOptions, TreeReader, TreeView};

/// A ring of `capacity` slots of `slot` bytes, its descriptors at 8 and its
/// payloads past them, in a buffer of its own.
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
    let ack = FrameFlag::ACKREQ as u32;
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
fn ring_a_packet_written_in_place_reads_as_the_one_pushed_and_one_past_a_slot_is_refused_whatever_the_ring_holds() {
    use xpute_core::wire::xtp::PacketWriter;
    let (r, _buf) = ring(8, 64);
    let write = |n: u32| {
        move |out: &mut [u8]| {
            let mut w = PacketWriter::new(out, 2);
            w.u32(Some(n)).u64(Some((n as u64) << 40));
            w.finish()
        }
    };
    let mut t: TreeView<'_> = TreeView::new();
    t.branch(Some(&mut |b| {
        b.u32(Some(5)).u64(Some(5 << 40));
    }));
    let pushed = encoder().encode(&t, TreeEncoderOptions::default()).unwrap();
    assert_eq!(r.push_in_place(1, 2, 0, 0, write(5)), Errno::OK);
    let at = r.peek() as u32;
    assert_eq!(r.packet(at).unwrap().unwrap(), pushed.as_slice());

    // The same packet is refused the same way with a message in the ring and
    // with none: a slot is a slot, so what fits does not depend on what was
    // sent before it.
    let long = "x".repeat(200);
    let too_long = |out: &mut [u8]| {
        let mut w = PacketWriter::new(out, 1);
        w.str(Some(&long));
        w.finish()
    };
    let tail = r.tail();
    assert_eq!(r.push_in_place(1, 2, 0, 0, too_long), Errno::EMSGSIZE);
    assert_eq!(r.tail(), tail, "and nothing written");
    r.advance();
    assert_eq!(r.push_in_place(1, 2, 0, 0, too_long), Errno::EMSGSIZE, "an empty ring answers the same");

    assert_eq!(r.push_in_place(3, 4, 0, -1, write(6)), Errno::OK);
    let at = r.peek() as u32;
    assert_eq!((r.tag(at), r.cmd(at), r.result(at)), (3, 4, -1));
    let reader = TreeReader::new(r.packet(at).unwrap().unwrap()).unwrap().read_branch().unwrap();
    assert_eq!(reader.at(0).unwrap().get_number().unwrap(), 6.0);
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
    r.words()[(at + FRAME_PACKET) as usize] = 4;
    assert!(r.packet(at).is_err(), "outside a slot");
}
