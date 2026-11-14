// xpute-native-host/stop.rs

//! The stop the host serves the guest (wit/stop/stop.wit): the guest's report
//! of a broken invariant, logged and told the app by its runtime
//! (xpute-native-runtime listener.rs `tell_stop`), before the guest stops
//! (stop.ts). The host's own failures are its runtime's to tell, as a
//! stop and a trap (xpute-native-runtime native.rs `hook`); a guest's trap is
//! told by its runtime too (link.rs `trap`), as a page's stop and trap are two
//! messages.

use std::rc::Rc;

use xpute_kit::status::errno::Errno;
use xpute_native_runtime::abi;
use xpute_native_runtime::listener::{self, Stop};
use xpute_native_runtime::log;

struct Report;

impl abi::stop::Host for Report {
    fn stopped(&self, errnum: i32, file: String, line: u32, n1: u64, n2: u64) -> Result<(), Errno> {
        log(&format!(
            "[guest] stopped: {errnum} at {file}:{line} ({n1}, {n2})"
        ));
        listener::tell_stop(Stop {
            errnum,
            file,
            line,
            n1,
            n2,
        });
        Ok(())
    }
}

/// Mounts the stop.
pub fn serve() {
    abi::stop::mount(Rc::new(Report));
}
