// xpute-native-host/lib.rs

//! The host's end of xpute on a native device, paired file by file with
//! @xpute/web-host: what it serves the guest (`stop`, `clock`). It runs in
//! its runtime (xpute-native-runtime), on the guest's thread, and mounts what
//! it serves on the runtime's link, as a page's worker composes
//! @xpute/web-host's over the link it instantiated.

#![cfg(not(target_arch = "wasm32"))]

pub mod clock;
pub mod stop;

use xpute_kit::status::bug::Bug;
use xpute_kit::status::errno::Errno;
use xpute_native_runtime::{link, post_task};

/// Makes a panic of the host's own, on any thread, a stop: it is logged and
/// handed to the platform, and the link is trapped on the guest's thread, so
/// that no answer the dead thread owed leaves the guest waiting. A guest
/// installs its own hook over this one and hands back what is not its own
/// (xpute-guest link.rs `in_guest`). A bug panics with itself, which no report
/// outside the guest took.
pub fn hook() {
    std::panic::set_hook(Box::new(|info| {
        match info.payload().downcast_ref::<Bug>() {
            Some(b) => stop::report_own(b.errnum as i32, b.file, b.line),
            None => {
                let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
                stop::report_own(Errno::ENOTRECOVERABLE as i32, file, line);
            }
        }
        post_task(Box::new(link::trap));
    }));
}
