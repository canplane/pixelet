// @xpute/core/ipc/ring.test.ts

use super::*;

#[test]
fn ring_entries_come_out_in_order_across_the_wrap_and_a_full_ring_refuses() {
    let buf = ArrayBuffer::new((ring_words(4) * 4 + 8) as usize);
    let r = Ring::init(buf, 8, 4);
    for round in 0..3u32 {
        for k in 0..4 {
            assert_eq!(r.push(round * 4 + k, 0, &[(round * 4 + k) * 10]), Errno::OK);
        }
        assert_eq!(r.push(99, 0, &[]), Errno::EAGAIN, "a full ring refuses");
        for k in 0..4 {
            let at = r.peek();
            assert!(at >= 0);
            let at = at as usize;
            assert_eq!([r.words[at + RING_USER as usize], r.words[at + RING_ARGS as usize]], [round * 4 + k, (round * 4 + k) * 10]);
            r.advance();
        }
        assert_eq!(r.peek(), -1);
    }
    assert_eq!([r.head(), r.tail()], [12, 12], "positions only grow");
}
