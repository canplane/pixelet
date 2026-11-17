// xpute-host/clock.rs

//! The clocks the host implements for the guest
//! (wit/clocks/monotonic-clock.wit, wit/clocks/system-clock.wit), in
//! nanoseconds, as clock.ts: the monotonic one from the host's start, its waits
//! on its runtime's timers (runtime.rs `after`), and the system clock from the
//! Unix epoch. A platform's timestamp of an event takes its mark on the
//! monotonic clock by `mark`, as the web's does by clock.ts's.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use xpute_kit::status::errno::Errno;
use xpute_link::{Return, Side};

use crate::abi::clocks::monotonic_clock;
use crate::abi::clocks::system_clock::{self, Instant as SystemInstant};
use crate::runtime::runtime;

/// Where the monotonic time counts from, as the web's `performance.now()`
/// counts from its time origin: the host's first mount of the clocks.
static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);

/// The monotonic clock's mark of `at`, in nanoseconds from `ORIGIN`.
pub fn mark(at: Instant) -> u64 {
    at.saturating_duration_since(*ORIGIN).as_nanos() as u64
}

struct Monotonic;

impl monotonic_clock::Host for Monotonic {
    fn now(&self) -> Result<u64, Errno> {
        Ok(mark(Instant::now()))
    }

    /// Answers once `how_long` has passed; 0, at the loop's next pass. A wait
    /// its caller cancels stops its timer and answers ECANCELED, as clock.ts's
    /// does.
    fn wait_for(&self, how_long: u64, task_return: Return<()>) {
        let timer = Rc::new(Cell::new(None));
        task_return.on_cancel({
            let timer = timer.clone();
            move || {
                if let Some(r) = timer.get().and_then(runtime().clear) {
                    r.answer(Err(Errno::ECANCELED));
                }
            }
        });
        timer.set(Some((runtime().after)(
            Duration::from_nanos(how_long),
            task_return,
        )));
    }
}

struct System;

impl system_clock::Host for System {
    fn now(&self) -> Result<SystemInstant, Errno> {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Ok(SystemInstant {
            seconds: t.as_secs() as i64,
            nanoseconds: t.subsec_nanos(),
        })
    }
}

/// Mounts the clocks on `side`.
pub fn mount(side: &'static Side) {
    LazyLock::force(&ORIGIN);
    monotonic_clock::mount(side, Rc::new(Monotonic));
    system_clock::mount(side, Rc::new(System));
}
