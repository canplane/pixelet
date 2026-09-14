// xpute_core/ipc/frame.test.rs
// (no pair: frame.ts has no test file)

use super::*;
use crate::vector::Vector;

const VECTOR: &str = include_str!("../../vector/ipc/frame.tsv");

#[test]
fn frame_lays_out_what_the_typescript_lays_out() {
    let v = Vector::load(VECTOR);
    v.each("tx", |k| {
        let id = make_tx_id(v.u32(&format!("{k}.w")), v.u32(&format!("{k}.s")));
        assert_eq!(
            (id, tx_worker_id(id), tx_seq(id)),
            (v.u32(&format!("{k}.id")), v.u32(&format!("{k}.worker")), v.u32(&format!("{k}.seq"))),
            "{k}"
        );
    });
    v.each("align8", |k| {
        assert_eq!(align8(v.u64(&format!("{k}.n"))), v.u64(&format!("{k}.a")), "{k}");
    });
    v.each("lay_out", |k| {
        let frame = Frame::new(ArrayBuffer::new(v.u32(&format!("{k}.bytes")) as usize));
        frame.begin(make_tx_id(3, 77), 0x1234, 2);
        let (args, body) = (v.u32(&format!("{k}.args")), v.u32(&format!("{k}.body")));
        // A throw there is a panic here.
        let at = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frame.lay_out(args, body))).map_or("throws".to_string(), |at| at.to_string());
        assert_eq!(at, v.s(&format!("{k}.at")), "{k}.at");
        let words: Vec<u32> = (0..8).map(|j| v.u32(&format!("{k}.words.{j}"))).collect();
        assert_eq!(TypedArray::<u32>::over(frame.buf.clone(), 0, 8).as_slice(), &words[..], "{k}.words");
    });
}
