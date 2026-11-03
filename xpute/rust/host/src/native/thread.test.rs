// xpute-host/native/thread.test.rs

use super::*;

/// A guest that stops ends its thread and not the process: the call in it and every
/// later one come back with none.
#[test]
fn stopped_ends_the_guest_alone() {
    start("guest", 1 << 20, || {});
    assert_eq!(run(|| 1), Some(1));
    assert_eq!(run::<()>(|| stopped()), None);
    assert_eq!(run(|| 2), None);
}
