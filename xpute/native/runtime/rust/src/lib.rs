// xpute-native-runtime/lib.rs

//! xpute's runtime on a native device, the environment its host runs in, as
//! @xpute/web-runtime and the browser are a page's. The runtime runs on the
//! guest's thread, as a page's worker holds the runtime and the guest
//! together: that thread waits on one queue and enters the guest when work
//! arrives (`event_loop`), and every other thread only waits long and pushes
//! into the queue. It gives its host (xpute-native-host) what the host needs
//! of it (xpute-native-host runtime.rs `Runtime`): the way back to the guest's
//! thread, the timers there, where a stop is told, and its writer, which
//! alone writes to the platform's output (`log`).
//!
//! It starts the guest from what the program lists (native.rs `Boot`), as
//! @xpute/web-runtime starts a page's: the program's servers,
//! xpute-native-host's among them, each mounting on the link what it serves,
//! and the interfaces the guest's world imports, which are all mounted
//! before the guest is entered. It runs the steps itself, in their order:
//! the servers, what the UI serves (`remote`), the guest's first entry
//! (`link::init`), and the loop for the guest's life. It exports itself to
//! the platform's module over the program's start (`native`, `native_abi!`),
//! as @xpute/web-runtime is the page's.
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

/// A method from the UI of `generation` (remote.rs `accept`), for the
/// guest's thread: `m` of one of the guest's objects `h`, its request `tag`
/// the UI's. Called on the JavaScript thread.
pub fn post(generation: u32, tag: u32, h: u32, m: u32, pkt: Option<Vec<u8>>) {
    event_loop::post_task(Box::new(move || remote::recv(generation, tag, h, m, pkt)));
}

/// The answer of the UI of `generation` to the request `tag` of the guest's
/// that it serves later, for the guest's thread. Called on the JavaScript
/// thread.
pub fn answer(generation: u32, tag: u32, result: i32, data: Option<Vec<u8>>) {
    event_loop::post_task(Box::new(move || {
        remote::answer(generation, tag, result, data)
    }));
}

/// The app was hidden or shown again, from the platform's lifecycle.
pub fn life(shown: bool) {
    event_loop::post_task(Box::new(move || life::tell(shown)));
}
