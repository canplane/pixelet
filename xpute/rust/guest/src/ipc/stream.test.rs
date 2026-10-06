// xpute-guest/ipc/stream.test.rs

use super::*;

fn stream(words: usize) -> Stream {
    Stream::new(Box::leak(vec![0u32; words].into_boxed_slice()), 1)
}

#[test]
fn a_transaction_past_the_range_is_dropped_whole_and_the_next_one_records() {
    let mut s = stream(16);
    s.record(1, &[7], &[], None);
    let before = s.left();
    s.begin();
    s.record(2, &[1, 2], &[], None);
    s.record(3, &[0; 12], &[], None);
    s.record(4, &[], &[], None);
    assert!(!s.commit(), "a record past the range drops the transaction");
    assert_eq!(s.left(), before, "the stream is as it was at `begin`");
    s.begin();
    s.record(5, &[1], &[], None);
    assert!(s.commit(), "a transaction that fits is kept");
    assert_eq!(s.left(), before - record_bytes(1, None));
}
