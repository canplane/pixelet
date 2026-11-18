// xpute-runtime-native/lib.rs

//! xpute's native runtime, the environment its host runs in, as
//! @xpute/runtime-web and the browser are the web's. The runtime runs on the
//! guest's thread, as the web worker holds the runtime and the guest together:
//! that thread waits on one queue and enters the guest when work arrives
//! (`event_loop`), and every other thread only waits long and pushes into the
//! queue. It gives its host (xpute-host) what the host needs of it (xpute-host
//! runtime.rs `Runtime`): the way back to the guest's thread, the timers there,
//! where a stop is told, and its writer, which alone writes to the platform's
//! output (`log`).
//!
//! It starts the guest from what the program lists (native.rs `Boot`), as
//! @xpute/runtime-web starts the web's: the program's mounts, xpute-host's
//! among them, each mounting on the link its implementations, and the
//! interfaces the guest's world imports, which are all mounted before the
//! guest is entered. It runs the steps itself, in their order: the mounts,
//! what the UI implements (`remote`), the guest's first entry (`link::init`),
//! which an import with no implementation and a function without `async` the
//! UI implements refuse, and the loop for the guest's life. It exports itself
//! to the platform's module over the program's start (`native`,
//! `native_abi!`), as @xpute/runtime-web is the web's.
//!
//! It makes the host's side of the link over the guest (`link`), from
//! xpute-link's machine (`Side`), and takes the rest of the contract both
//! sides take from it: a frame's words (`frame`), a URI's tokens
//! (`abi::types`) and how an address is read (`mem`), which is the
//! same in one process. It never calls xpute-guest, the
//! guest's own side.

#![cfg(not(target_arch = "wasm32"))]

mod event_loop;
#[cfg(target_os = "android")]
pub mod jni;
mod life;
mod link;
pub mod listener;
mod log;
pub mod native;
pub mod remote;

use xpute_host::abi::log::Level;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// A method from the UI of the handle `ui` (remote.rs `accept`), for the
/// guest's thread: `m` of one of the guest's objects `h`, its request `tag`
/// the UI's. Called on the JavaScript thread.
pub fn post(ui: u32, tag: u32, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    event_loop::post_task(Box::new(move || remote::recv(ui, tag, h, m, pkt)));
}

/// The answer of the UI of the handle `ui` to the request `tag` of the
/// guest's that it answers, for the guest's thread. Called on the JavaScript
/// thread.
pub fn answer(ui: u32, tag: u32, result: i32, data: Option<Vec<u8>>) {
    event_loop::post_task(Box::new(move || remote::answer(ui, tag, result, data)));
}

/// The app was hidden or shown again, from the platform's lifecycle.
pub fn life(shown: bool) {
    event_loop::post_task(Box::new(move || life::tell(shown)));
}

/// A log record of the UI's: `msg` at `level`, about `context`, written
/// by the runtime's writer (`log`) as the host's are. Called on the
/// JavaScript thread.
pub fn log(level: u32, context: &str, msg: &str) {
    log::log(Level::of(level).or_bug(Errno::EINVAL), context, msg);
}
