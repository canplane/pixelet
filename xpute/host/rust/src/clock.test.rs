// xpute-host/clock.test.rs

//! A wait its caller cancels stops its timer and answers ECANCELED, and a
//! wait answers once its time has passed (clock.rs `wait_for`), as clock.ts's
//! do.

use std::sync::Mutex;

use xpute_guest::abi::clocks::monotonic_clock::{HANDLE, WAIT_FOR};
use xpute_guest::link::{cancel, send};
use xpute_host::clock;
use xpute_kit::status::errno::Errno;
use xpute_link::{Lower, OPEN, ROOT};
use xpute_runtime_native::native::Boot;

#[path = "../../../runtime/native/src/app.test.rs"]
mod app;

/// The answers the guest heard: its wait's number, and the result.
static ANSWERS: Mutex<Vec<(u32, i32)>> = Mutex::new(Vec::new());

/// Waits `ns` nanoseconds, its answer heard as `n`'s.
fn wait(n: u32, ns: u64) -> u32 {
    let args = move |w: &mut Lower| w.u64(0, ns);
    let answer = move |r, _| ANSWERS.lock().unwrap().push((n, r));
    send(HANDLE, WAIT_FOR, Some((8, &args)), Some(Box::new(answer)))
}

// The guest waits an hour and cancels it, and waits a millisecond.
xpute_guest::link!(
    4096,
    || {
        cancel(wait(1, 3_600_000_000_000));
        wait(2, 1_000_000);
    },
    || {},
    || false
);

fn boot(_: std::path::PathBuf) -> Boot {
    Boot {
        stack_sz: 1 << 20,
        mounts: vec![Box::new(clock::mount)],
        imports: &[("monotonic-clock", HANDLE)],
        calls: &[],
    }
}

#[test]
fn cancel_stops_wait() {
    let ui = app::start(boot, &[]);
    app::until("the waits answered", || ANSWERS.lock().unwrap().len() == 2);
    assert_eq!(
        *ANSWERS.lock().unwrap(),
        [(1, Errno::ECANCELED as i32), (2, 0)]
    );
    let mut msgs = Vec::new();
    app::post(ui, 1, ROOT, OPEN, Some(app::lowered(&[])));
    assert_eq!(app::answer_to(ui, 1, &mut msgs), Errno::ENOENT as i64);
    assert_eq!(app::exits(), []);
}
