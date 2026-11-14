// xpute-native-host/clock.rs

//! The clocks the host serves the guest (wit/clocks/monotonic-clock.wit,
//! wit/clocks/system-clock.wit), in nanoseconds, as clock.ts: the monotonic one
//! from the host's start, its waits on the event loop's own timers, and the
//! system clock from the Unix epoch.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use xpute_kit::status::errno::Errno;

use xpute_link::Return;
use xpute_native_runtime::abi::clocks::monotonic_clock;
use xpute_native_runtime::abi::clocks::system_clock::{self, Instant as SystemInstant};
use xpute_native_runtime::event_loop;

/// Where the monotonic time counts from, as a page's `performance.now()`
/// counts from its time origin.
struct Monotonic {
    origin: Instant,
}

impl monotonic_clock::Host for Monotonic {
    fn now(&self) -> Result<u64, Errno> {
        Ok(self.origin.elapsed().as_nanos() as u64)
    }

    /// Answers once `how_long` has passed; 0, at the loop's next pass. A wait
    /// its caller cancels stops its timer and answers ECANCELED, as clock.ts's
    /// does.
    fn wait_for(&self, how_long: u64, task_return: Return<()>) {
        let timer = Rc::new(Cell::new(None));
        task_return.on_cancel({
            let timer = timer.clone();
            move || {
                if let Some(r) = timer.get().and_then(event_loop::clear) {
                    r.answer(Err(Errno::ECANCELED));
                }
            }
        });
        timer.set(Some(event_loop::after(
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

pub fn serve() {
    monotonic_clock::mount(Rc::new(Monotonic {
        origin: Instant::now(),
    }));
    system_clock::mount(Rc::new(System));
}
