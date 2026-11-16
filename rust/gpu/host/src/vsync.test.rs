// pxl-gpu-host/vsync.test.rs

use std::cell::RefCell;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;

use xpute_host::runtime::{self, Runtime};

use super::*;
use crate::abi::vsync::Host;

/// A runtime whose guest's thread is the test's own: a task posted runs at
/// once.
fn init_runtime() {
    runtime::init(Runtime {
        post: |task| task(),
        after: |_, _| unreachable!(),
        clear: |_| unreachable!(),
        stopped: |_| unreachable!(),
        log: |_, _, _| {},
    });
}

/// A `next` waiting from `after`, and what it was answered.
fn wait(after: u64) -> Rc<RefCell<Option<Frame>>> {
    let got = Rc::new(RefCell::new(None));
    let into = Rc::clone(&got);
    Vsync.next(
        after,
        Return::new(
            Box::new(|_| {}),
            Box::new(move |frame| *into.borrow_mut() = Some(frame.unwrap())),
        ),
    );
    got
}

/// A millisecond, in nanoseconds.
const MS: u64 = 1_000_000;

/// The display link calls at every frame, so the host has each frame's gap
/// whether or not a wait was pending at the frame before, and a wait is
/// answered at the first frame from its `after`, never before.
#[test]
fn a_wait_is_answered_from_its_after_with_the_frames_gap() {
    init_runtime();
    let first = wait(0);
    vsync(0);
    let first = first.borrow().clone().unwrap();
    assert_eq!(first.gap, 0, "the host's first frame has no gap");

    // Frames that started 3 and 2 ms ago, once the clock has run past both;
    // no wait is pending at the first.
    sleep(Duration::from_millis(10));
    vsync(3 * MS);
    let late = wait(u64::MAX);
    let next = wait(0);
    vsync(2 * MS);
    let next = next.borrow().clone().unwrap();
    // The gap is since the frame just before, which no wait reached.
    assert!(next.gap > MS / 2 && next.gap < 3 * MS / 2, "{}", next.gap);
    assert_eq!(late.borrow().as_ref(), None);
}
