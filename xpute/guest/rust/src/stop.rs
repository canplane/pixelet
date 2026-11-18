// xpute-guest/stop.rs

//! The guest's report of its own failures to the host, by the stop the host
//! implements (wit/stop/stop.wit), as cpp/xpute/guest/stop.hpp is a C++
//! guest's: a bug's errno, place, numbers and message, or a panic's place and
//! its own message.

use xpute_kit::status::bug;
use xpute_kit::status::errno::Errno;

use crate::abi::stop;
use crate::link::{in_guest, in_init};

/// Tells the host a bug or a panic in the guest's own code, by its stop,
/// before the guest stops; `link!` runs it as the guest's first export
/// starts, so that it covers the guest's whole run, `init` too, where the
/// failure is told by the trap alone, as on the web. Both hooks are the
/// process's natively, so what is not the guest's own (another thread's,
/// or the host's while the guest is out on `link_out`) goes on to the hook
/// before, its runtime's (xpute-runtime-native native.rs `hook`), as it
/// would have. A bug is told with its message, where it has one; a panic
/// `bug` did not raise, the standard library's or a dependency's, with its
/// own.
pub fn hook() {
    bug::set_report(|errnum, at, n1, n2, msg| {
        if in_guest() {
            let _ = stop::stopped(errnum as i32, at.file(), at.line(), n1, n2, msg);
        }
    });
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if in_init() {
            return;
        }
        if !in_guest() {
            return prev(info);
        }
        // A bug's own panic, which its report told already.
        if info.payload().is::<bug::XputeBug>() {
            return;
        }
        let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
        let _ = stop::stopped(Errno::ENOTRECOVERABLE as i32, file, line, 0, 0, msg);
    }));
}
