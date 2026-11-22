// xpute-guest/link.test.rs

//! The host of the guest's own tests: a `link_out` that keeps each frame the
//! guest lays, on its thread, and answers 0.

use std::cell::RefCell;

use xpute_golden::Golden;
use xpute_link::frame::Frame;

use super::*;
use crate::CAP_MIN;

thread_local! {
    /// The frames this thread's guest laid, each with its packet.
    static LAID: RefCell<Vec<(Frame, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
extern "C" fn link_out(at: u64) -> i32 {
    let f = Frame::at(at);
    // SAFETY: the packet of the frame the guest just laid at `at`.
    let pkt = unsafe { mem::read(at + u64::from(HDR_SZ), f.pkt_len) }.to_vec();
    LAID.with_borrow_mut(|l| l.push((f, pkt)));
    0
}

/// Lays this thread's link, runs `f`, and answers the frames the guest laid
/// meanwhile.
pub(crate) fn laid_by(f: impl FnOnce()) -> Vec<(Frame, Vec<u8>)> {
    init(CAP_MIN, || {}, || false);
    f();
    LAID.take()
}

/// The record's message past the cap, and what the cut leaves of it
/// (kit/spec/golden/status/msg.tsv, which `cut_msg`'s own tests read).
pub(crate) fn long_and_cut() -> (String, String) {
    let g = Golden::load(include_str!("../../../kit/spec/golden/status/msg.tsv"));
    let text = |k: &str| String::from_utf8(g.bytes(k)).expect("UTF-8");
    (text("straddle.in"), text("straddle.out"))
}
