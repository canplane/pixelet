// xpute-native-runtime/listener.rs

//! What the app hears from the runtime, through the functions it sets, as a
//! Wayland client's listener hears its events: a batch waiting for the UI
//! (`notify_on`), the guest's stop (`stopped_on`) and its trap (`trap_on`);
//! and what the app was told of the guest's end, told again to each new UI
//! (`tell_end`). It names nothing else of the runtime: the link tells it a
//! trap (link.rs `trap`), the host the guest's stop (xpute-native-host
//! stop.rs) and the runtime's hook a failure of the host's (native.rs
//! `hook`), as a page's runtime posts its own (@xpute/web-runtime worker.ts
//! `tell`).

use std::sync::{Mutex, MutexGuard};

/// The report of a broken invariant before the guest stops: the guest's
/// (wit/stop/stop.wit), which the host that serves the stop hands the runtime
/// (xpute-native-host stop.rs), or the host's own failure (native.rs `hook`),
/// which the runtime tells the app (`tell_stop`).
pub struct Stop {
    pub errnum: i32,
    pub file: String,
    pub line: u32,
    pub n1: u64,
    pub n2: u64,
}

/// What tells the app a batch waits.
pub type Tell = extern "C" fn();

/// What tells the app the guest stopped: its errno, its file's UTF-8 and
/// length, its line and its two numbers.
pub type Stopped = extern "C" fn(i32, *const u8, usize, u32, u64, u64);

/// What tells the app the guest trapped: what stopped it, its UTF-8 and
/// length, as a page's trap carries its error (@xpute/web-runtime `Trap`).
pub type Trapped = extern "C" fn(*const u8, usize);

static NOTIFY: Mutex<Option<Tell>> = Mutex::new(None);
static STOPPED: Mutex<Option<Stopped>> = Mutex::new(None);
static TRAP: Mutex<Option<Trapped>> = Mutex::new(None);

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
    *TRAP.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

/// Tells the app a batch waits (remote.rs `flush`).
pub(crate) fn notify() {
    if let Some(f) = *NOTIFY.lock().unwrap_or_else(|e| e.into_inner()) {
        f();
    }
}

/// What the app was told of the guest's end: the last stop reported, as the
/// UI keeps it, and what trapped the guest, told again to each new UI
/// (`tell_end`).
struct End {
    stop: Option<Stop>,
    trap: Option<String>,
}

static END: Mutex<End> = Mutex::new(End {
    stop: None,
    trap: None,
});

fn end() -> MutexGuard<'static, End> {
    END.lock().unwrap_or_else(|e| e.into_inner())
}

fn call_stopped(s: &Stop) {
    if let Some(f) = *STOPPED.lock().unwrap_or_else(|e| e.into_inner()) {
        f(s.errnum, s.file.as_ptr(), s.file.len(), s.line, s.n1, s.n2);
    }
}

fn call_trapped(err: &str) {
    if let Some(f) = *TRAP.lock().unwrap_or_else(|e| e.into_inner()) {
        f(err.as_ptr(), err.len());
    }
}

/// Tells the app the guest stopped, with the report of the broken
/// invariant: the guest's, which the host that serves the stop hands it
/// (xpute-native-host stop.rs), or the host's own failure (native.rs
/// `hook`), as @xpute/web-runtime worker.ts `tell_stop` tells a page.
pub fn tell_stop(s: Stop) {
    let mut end = end();
    call_stopped(&s);
    end.stop = Some(s);
}

/// Tells the app the guest trapped, with what stopped it (link.rs `trap`).
pub(crate) fn tell_trap(err: &str) {
    let mut end = end();
    call_trapped(err);
    end.trap = Some(err.to_owned());
}

/// Tells a new UI what the app was told of the guest's end, as the first heard
/// it (native.rs `start`): a stop told again lands before a trap told
/// meanwhile.
pub(crate) fn tell_end() {
    let end = end();
    if let Some(s) = &end.stop {
        call_stopped(s);
    }
    if let Some(err) = &end.trap {
        call_trapped(err);
    }
}
