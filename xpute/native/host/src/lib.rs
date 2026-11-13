// xpute-native-host/lib.rs

//! The host's end of xpute on a native device, paired file by file with
//! @xpute/web-host. The host runs on the guest's thread, as a page's worker
//! holds the host and the guest together: that thread waits on one queue
//! and enters the guest when work arrives (`event_loop`), and every other
//! thread only waits long and pushes into the queue.
//!
//! It offers pieces, which the program composes on the guest's thread in the
//! order they run, as a page's worker composes @xpute/web-host's: the
//! host's panic hook (`hook`), what the host serves (`stop`,
//! `clock`, `remote`, and the program's own, over the `disk` where
//! it keeps files), the guest's first entry (`link::init`), then the
//! answers due, the UI's batch and the loop
//! (`link::run_due`, `remote::flush`, `event_loop::run`).
//!
//! It takes the link, the contract both sides take, from xpute-link: a
//! frame's words (`frame`), the link machine (`Side`), a URI's tokens and a
//! lent span (`abi::types`), the handle table (`resource`), and the guest's
//! memory (`mem`), which is the same in one process. It never calls
//! xpute-guest, the guest's own side.

#![cfg(not(target_arch = "wasm32"))]

/// The bindings of xpute's packages on a native device's host's side, a
/// module to a package, generated from their WIT (xpute-gen-targets).
pub mod abi;
pub mod clock;
pub mod disk;
pub mod event_loop;
#[cfg(target_os = "android")]
pub mod jni;
pub mod life;
pub mod link;
mod log;
pub mod remote;
pub mod stop;
pub mod wait;

pub use event_loop::{done, job, post_task, Task};
pub use log::log;

use xpute_kit::status::bug::Bug;
use xpute_kit::status::errno::Errno;

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
        post_task(Box::new(link::trapped));
    }));
}

/// A method from the UI, for the guest's thread: `m` of one of the guest's
/// objects `h`, its request `tag` the UI's (remote.rs). Called on the
/// JavaScript thread.
pub fn post(tag: u32, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    event_loop::post_task(Box::new(move || remote::recv(tag, h, m, pkt)));
}

/// The app was hidden or shown again, from the platform's lifecycle.
pub fn life(shown: bool) {
    event_loop::post_task(Box::new(move || life::tell(shown)));
}
