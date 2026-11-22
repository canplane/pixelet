// xpute-guest/stop.test.rs

use std::panic::{AssertUnwindSafe, catch_unwind};

use xpute_link::Lift;
use xpute_link::frame::Frame;

use super::*;
use crate::link::boot;
use crate::link::test::{laid_by, long_and_cut};

/// The message of the stop the guest told in the one frame of `laid`, whose
/// errno is `errnum`.
fn told_msg(laid: &[(Frame, Vec<u8>)], errnum: Errno) -> String {
    let [(f, pkt)] = laid else {
        panic!("one frame, not {}", laid.len());
    };
    assert_eq!((f.handle, f.method), (stop::HANDLE, stop::STOPPED));
    let args = Lift::new(pkt);
    assert_eq!(args.i32(0), Ok(errnum as i32));
    assert_eq!(args.u8(32), Ok(1), "a message");
    args.str(36).expect("the message").to_owned()
}

/// A bug's message past the cap is told cut as the record of the cut holds
/// it.
#[test]
fn stop_tells_a_bug_with_its_msg_cut() {
    let (long, cut) = long_and_cut();
    let long: &'static str = long.leak();
    let laid = laid_by(|| {
        hook();
        let bugged = catch_unwind(AssertUnwindSafe(|| {
            boot(|| bug::bug_with_msg(Errno::EIO, 1, 2, long));
        }));
        assert!(bugged.is_err());
    });
    assert_eq!(told_msg(&laid, Errno::EIO), cut);
}

/// A panic's message past the cap is told cut as the record of the cut holds
/// it.
#[test]
fn stop_tells_a_panic_with_its_msg_cut() {
    let (long, cut) = long_and_cut();
    let laid = laid_by(|| {
        hook();
        let panicked = catch_unwind(AssertUnwindSafe(|| boot(|| panic!("{long}"))));
        assert!(panicked.is_err());
    });
    assert_eq!(told_msg(&laid, Errno::ENOTRECOVERABLE), cut);
}
