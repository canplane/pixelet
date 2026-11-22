// xpute-kit/status/bug.test.rs

use super::*;
use xpute_golden::Golden;

/// The bug `bug_with_msg` stops with, made with `msg`.
fn stopped(msg: &'static str) -> XputeBug {
    let payload =
        std::panic::catch_unwind(|| bug_with_msg(Errno::EIO, 7, 0, msg)).expect_err("a bug stops");
    *payload.downcast::<XputeBug>().expect("the bug itself")
}

/// A bug's message past the cap is cut as the record of the cut holds it
/// (kit/spec/golden/status/msg.tsv), a part of the static string itself, and
/// an empty one is none.
#[test]
fn bug_msg_is_cut() {
    let g = Golden::load(include_str!("../../../spec/golden/status/msg.tsv"));
    let text = |k: &str| String::from_utf8(g.bytes(k)).expect("UTF-8");
    let long: &'static str = text("straddle.in").leak();
    let cut = text("straddle.out");
    let b = stopped(long);
    assert_eq!((b.errnum, b.a, b.file), (Errno::EIO, 7, file!()));
    assert_eq!(b.msg, Some(cut.as_str()));
    assert!(core::ptr::eq(b.msg.unwrap().as_ptr(), long.as_ptr()));
    assert_eq!(stopped("").msg, None);
    assert_eq!(stopped("full").msg, Some("full"));
}
