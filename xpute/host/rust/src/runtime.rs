// xpute-host/runtime.rs

//! What the host needs of the runtime it runs in, which the runtime implements
//! and gives it once, on the guest's thread, before it mounts any
//! implementation (`init`), as the web runtime gives @xpute/host its own: the
//! way back to the guest's thread (`post`), the timers there (`after`,
//! `clear`), where the guest's stop is told (`stopped`), and where a log record
//! is written (`log`).

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::Return;

use crate::abi::log::Level;

/// A piece of work for the guest's thread.
pub type Task = Box<dyn FnOnce() + Send>;

/// A timer `after` set, when it is due and a count that keeps it apart, until
/// it has answered or is cleared.
pub type Timer = (Instant, u64);

/// The report of a broken invariant before the guest stops: the guest's
/// (wit/stop/stop.wit), which the host that implements the stop hands its
/// runtime (stop.rs), or the host's own failure, which its runtime
/// reports itself. Its message is a panic's own, or the bug's where it has
/// one.
pub struct Stop {
    pub errnum: i32,
    pub file: String,
    pub line: u32,
    pub n1: u64,
    pub n2: u64,
    pub msg: Option<String>,
}

pub struct Runtime {
    /// Pushes a task for the guest's thread, from any thread: the way back
    /// for a host that keeps a thread of its own, which only the runtime that
    /// runs the guest's thread can give.
    pub post: fn(Task),
    /// Answers the `Return` once the time has passed, on the guest's thread,
    /// and returns the timer that will, as the web's `setTimeout` returns its
    /// id.
    pub after: fn(Duration, Return<()>) -> Timer,
    /// Clears a timer before it answers, as the web's `clearTimeout` does, and
    /// hands back what it would have answered; none once it has answered.
    pub clear: fn(Timer) -> Option<Return<()>>,
    /// Reports the guest's stop.
    pub stopped: fn(Stop),
    /// Writes a log record: its level, its context and its message.
    pub log: fn(Level, &str, &str),
}

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// Gives the host its runtime, once a process.
pub fn init(runtime: Runtime) {
    RUNTIME.set(runtime).ok().or_bug(Errno::EALREADY);
}

/// The runtime `init` gave, which nothing reads before it.
pub fn runtime() -> &'static Runtime {
    RUNTIME.get().or_bug(Errno::ENOTRECOVERABLE)
}
