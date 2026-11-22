// xpute-guest/log.test.rs

use xpute_link::Lift;

use super::*;
use crate::link::test::{laid_by, long_and_cut};

/// A record's context and its message past the cap are cut as the record of
/// the cut holds them, before the frame that tells them is laid.
#[test]
fn log_cuts_its_context_and_msg() {
    let (long, cut) = long_and_cut();
    let laid = laid_by(|| log(Level::Warn, &long, &long));
    let [(f, pkt)] = &laid[..] else {
        panic!("one frame, not {}", laid.len());
    };
    assert_eq!((f.handle, f.method), (log::HANDLE, log::LOG));
    let args = Lift::new(pkt);
    assert_eq!(args.str(4), Ok(cut.as_str()), "the context");
    assert_eq!(args.str(12), Ok(cut.as_str()), "the message");
}
