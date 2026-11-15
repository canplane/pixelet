// xpute-link/frame.test.rs

use super::*;
use crate::mem::test::alloc;
use xpute_golden::Golden;

const GOLDEN: &str = include_str!("../../spec/golden/frame.tsv");

/// Each recorded frame's fields lay its words, and its words read back as
/// its fields.
#[test]
fn matches_record() {
    let g = Golden::load(GOLDEN);
    let at = alloc(HDR_SZ);
    g.each("", |k| {
        let f = Frame {
            tag: g.u32(&format!("{k}.tag")),
            res: g.bool(&format!("{k}.res")),
            handle: g.u32(&format!("{k}.handle")),
            method: g.u32(&format!("{k}.method")),
            result: g.i32(&format!("{k}.result")),
            pkt_len: g.u32(&format!("{k}.pkt_len")),
        };
        f.lay(at);
        // SAFETY: the bytes `alloc` gave this test.
        let words = unsafe { mem::read(at, HDR_SZ) };
        assert_eq!(hex::encode(words), g.s(&format!("{k}.words")), "{k}.words");
        // SAFETY: as above.
        unsafe { mem::write(at, &g.bytes(&format!("{k}.words"))) };
        assert_eq!(Frame::at(at), f, "{k}");
    });
}
