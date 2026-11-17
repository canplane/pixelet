// xpute-runtime-native/listener.rs

//! What the app hears from the runtime, through the functions it sets, as a
//! Wayland client's listener hears its events: a batch waiting for the UI
//! (`notify_on`), the guest's stop (`stopped_on`) and its trap (`trap_on`),
//! the last of each kept for a UI that starts after it (`stop`, `trap`). It
//! names nothing else of the runtime: the runtime's link tells it a trap
//! (link.rs `trap`), the host hands the runtime the guest's stop (native.rs
//! `stopped`), and the runtime's hook a failure of the host's (native.rs
//! `hook`), each written and told here, as the web runtime posts its own (@xpute/runtime-web
//! worker.ts `tell`).

use std::sync::{Mutex, MutexGuard};

use xpute_host::runtime::Stop;

/// What tells the app a batch waits.
pub type Tell = extern "C" fn();

/// What tells the app the guest stopped: its errno, its file's UTF-8 and
/// length, its line and its two numbers.
pub type Stopped = extern "C" fn(i32, *const u8, usize, u32, u64, u64);

/// What tells the app the guest trapped: what stopped it, its UTF-8 and
/// length, as the web's trap carries its error (@xpute/runtime-web `Trap`).
pub type Trapped = extern "C" fn(*const u8, usize);

static NOTIFY: Mutex<Option<Tell>> = Mutex::new(None);
static STOPPED: Mutex<Option<Stopped>> = Mutex::new(None);
static TRAPPED: Mutex<Option<Trapped>> = Mutex::new(None);

/// Makes `f` be told that a batch waits for the UI (xpute.h `xpute_notify_on`).
pub fn notify_on(f: Option<Tell>) {
    *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

/// Makes `f` be told the guest's stop (xpute.h `xpute_stopped_on`).
pub fn stopped_on(f: Option<Stopped>) {
    *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

/// Makes `f` be told the guest's trap (xpute.h `xpute_trap_on`).
pub fn trap_on(f: Option<Trapped>) {
    *TRAPPED.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

/// Tells the app a batch waits (remote.rs `flush`).
pub(crate) fn notify() {
    if let Some(f) = *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) {
        f();
    }
}

/// What the app was last told of the guest's end: its stop and what trapped
/// it, none before each is told (`stop`, `trap`). Each told is leaked, a few
/// a process at most, so what a getter lends lives as long as the process.
struct End {
    stop: Option<&'static Stop>,
    trap: Option<&'static str>,
}

static END: Mutex<End> = Mutex::new(End {
    stop: None,
    trap: None,
});

fn end() -> MutexGuard<'static, End> {
    END.lock().unwrap_or_else(|e| e.into_inner())
}

/// The guest's last stop, or none before it stopped: a UI that starts after
/// it reads it so.
pub fn stop() -> Option<&'static Stop> {
    end().stop
}

/// What trapped the guest, or none before it trapped: a UI that starts after
/// the trap reads it so.
pub fn trap() -> Option<&'static str> {
    end().trap
}

fn call_stopped(s: &Stop) {
    if let Some(f) = *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) {
        f(s.errnum, s.file.as_ptr(), s.file.len(), s.line, s.n1, s.n2);
    }
}

fn call_trapped(err: &str) {
    if let Some(f) = *TRAPPED.lock().unwrap_or_else(|e| e.into_inner()) {
        f(err.as_ptr(), err.len());
    }
}

/// Tells the app the guest stopped, with the report of the broken
/// invariant: the guest's, which the host that implements the stop hands it
/// (native.rs `stopped`), or the host's own failure
/// (native.rs `hook`), as @xpute/runtime-web worker.ts `tell_stop` tells a
/// page. It is kept before it is told (`stop`), so a UI that reads none
/// hears it.
pub(crate) fn tell_stop(s: Stop) {
    let s: &'static Stop = Box::leak(Box::new(s));
    end().stop = Some(s);
    call_stopped(s);
}

/// Tells the app the guest trapped, with what stopped it (link.rs `trap`),
/// kept before it is told (`trap`).
pub(crate) fn tell_trap(err: &str) {
    let err: &'static str = err.to_owned().leak();
    end().trap = Some(err);
    call_trapped(err);
}
