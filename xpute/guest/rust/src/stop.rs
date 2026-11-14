// xpute-guest/stop.rs

//! The guest's report of its own failures to the host, by the stop the host
//! serves (wit/stop/stop.wit), as cpp/xpute/guest/stop.hpp is a C++ guest's.

use xpute_kit::status::bug;
use xpute_kit::status::errno::Errno;

use crate::abi::stop;
use crate::link::in_guest;

/// Tells the host a bug or a panic in the guest's own code, by its stop,
/// before the guest stops; `link!` runs it before the guest's boot. Both
/// hooks are the process's on a device, so what is not the guest's own
/// (another thread's, or the host's while the guest is out on `link_out`)
/// goes on to the hook before, the host's (xpute-native-host stop.rs
/// `hook`), as it would have.
pub fn hook() {
    bug::set_report(|errnum, at, n1, n2| {
        if in_guest() {
            let _ = stop::stopped(errnum as i32, at.file(), at.line(), n1, n2);
        }
    });
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !in_guest() {
            return prev(info);
        }
        // A bug's own panic, which its report told already.
        if info.payload().is::<bug::Bug>() {
            return;
        }
        let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
        let _ = stop::stopped(Errno::ENOTRECOVERABLE as i32, file, line, 0, 0);
    }));
}

