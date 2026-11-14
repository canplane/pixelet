// xpute-native-runtime/lib.rs

//! xpute's runtime on a native device, the environment its host runs in, as
//! @xpute/web-runtime and the browser are a page's. The runtime runs on the
//! guest's thread, as a page's worker holds the runtime and the guest
//! together: that thread waits on one queue and enters the guest when work
//! arrives (`event_loop`), and every other thread only waits long and pushes
//! into the queue.
//!
//! It offers pieces, which the program composes on the guest's thread in the
//! order they run, as a page's worker composes @xpute/web-runtime's: the
//! UI's stand-ins (`remote`), the guest's first entry (`link::init`), then
//! the answers due, the UI's batch and the loop (`link::run_due`,
//! `remote::flush`, `event_loop::run`). What the host serves is mounted on
//! its link (xpute-native-host).
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
pub mod event_loop;
#[cfg(target_os = "android")]
pub mod jni;
mod life;
pub mod link;
mod log;
pub mod remote;
pub mod wait;

pub use event_loop::{done, job, post_task, Task};
pub use log::log;

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
