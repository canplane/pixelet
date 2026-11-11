// xpute-core/host.rs

//! The core reaches its runtime through three functions, which the runtime
//! hands it at boot (`entry::BootInfo`). Each names a frame in the core's
//! memory by its offset: its words, then its packet (io/frame.rs). A page's
//! runtime gives them as the module's imports, a native one as functions of
//! its process.
//!
//! - `receive` writes the next frame the runtime sends the core into the
//!   `cap` bytes at `at`, and answers 1, or 0 when none waits.
//! - `send` takes the frame at `at`, and the `named_len` bytes at `named_at`
//!   that the frame names, before it returns: both are the core's again at
//!   once.
//! - `call` takes the frame at `at` and answers it before it returns, with a
//!   value or a negative errno. It never calls into the core.
//!
//! Reaching the runtime before it has handed them over is a bug.

use core::cell::Cell;

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

#[derive(Clone, Copy)]
pub struct Host {
    pub receive: fn(at: u32, cap: u32) -> i32,
    pub send: fn(at: u32, named_at: u32, named_len: u32),
    pub call: fn(at: u32) -> i32,
}

thread_local! {
    static HOST: Cell<Option<Host>> = const { Cell::new(None) };
}

pub fn set(host: Host) {
    HOST.with(|h| h.set(Some(host)));
}

fn get() -> Host {
    HOST.with(Cell::get).or_bug(Errno::ENOSYS)
}

pub fn receive(at: u32, cap: u32) -> i32 {
    (get().receive)(at, cap)
}

pub fn send(at: u32, named_at: u32, named_len: u32) {
    (get().send)(at, named_at, named_len)
}

pub fn call(at: u32) -> i32 {
    (get().call)(at)
}

/// Reads a runtime's answer as a count, or as the errno it refused with. A
/// negative code that names no errno means the runtime broke the protocol,
/// and reads as EPROTO.
pub fn answer_of(res: i32) -> Result<u32, Errno> {
    u32::try_from(res).map_err(|_| Errno::of(res).unwrap_or(Errno::EPROTO))
}
