// xpute-runtime-native/link.rs

//! The guest linked into the app, and the host's side of the link over it
//! (link/wit/types.wit), which the runtime makes and gives to each mount it
//! runs (native.rs `run`), as the web worker makes its side over the
//! module and gives it (worker.ts `instantiate`): the same machine as the
//! guest's (xpute-link link.rs `Side`) over the other box. It lays its frames
//! in the guest's inbox and crosses by entering the guest, which is linked
//! into the app as a module is on the web (worker.ts `Instance`): its three
//! exports, as C symbols, and `link_out`, which the guest imports. The
//! guest's memory is the process's, which the guest set before it laid its
//! link. The side is the guest thread's (`side`), where `link_out` finds it,
//! as the runtime's own remote.rs, life.rs and native.rs reach it, for the
//! tasks other threads post; a host is given it.
//!
//! A guest that stops, a Rust guest's panic or a C++ guest's bug, answers a
//! trap (frame.json5 `TRAP`, or `TRAP_AT` from init), as a module traps: the
//! runtime writes it and tells the platform, enters the guest no more, and
//! the UI stays (`trap`).

use xpute_host::abi::log::Level;
use xpute_kit::status::errno::Errno;
use xpute_link::{Handle, Layout, Side, TRAP, TRAP_AT};

use crate::listener;
use crate::log::log;

unsafe extern "C" {
    /// Lays the link and answers where its layout lies, or `TRAP_AT`.
    fn link_init() -> u64;
    /// The guest's first entry, which boots it: 0, or `TRAP`.
    fn link_boot() -> i32;
    /// Enters the guest with the frame laid in its inbox.
    fn link_in() -> i32;
}

thread_local! {
    /// The host's side, made on the guest's thread as it first asks for it,
    /// and kept for the thread's life, which is the process's.
    static SIDE: &'static Side = Box::leak(Box::new(Side::new(|l| l.inbox, cross_in)));
}

/// The host's side of the link. On the guest's thread.
pub(crate) fn side() -> &'static Side {
    SIDE.with(|s| *s)
}

/// The host's crossing: one guest entry with the frame laid in its inbox,
/// on the guest's thread, never while an entry runs; a trapped link lays and
/// crosses nothing (xpute-link link.rs `Side::out`).
fn cross_in(_at: u64) -> i32 {
    // SAFETY: the guest's export, on its thread, one entry at a time.
    match unsafe { link_in() } {
        TRAP => {
            trap(GUEST_TRAPPED);
            Errno::EPIPE as i32
        }
        result => result,
    }
}

/// What a trap out of the guest's export says: the guest told its failure
/// by its stop already, if it could.
const GUEST_TRAPPED: &str = "the guest trapped";

/// What a trap from a panic of the host's says, which the hook follows with
/// what the panic gives (native.rs `hook`).
pub(crate) const HOST_STOPPED: &str = "the host stopped";

/// Stops the link for good, where it has not trapped already, writes `err`,
/// what stopped it, and tells the platform, as the web worker tells its
/// trap with its error (worker.ts `tell_trap`). On the guest's thread.
pub(crate) fn trap(err: &str) {
    let s = side();
    if s.trapped() {
        return;
    }
    s.trap();
    log(Level::Error, "link", &format!("trapped: {err}"));
    listener::tell_trap(err);
}

/// Enters the guest first, once the host's own objects and what the UI
/// implements are mounted: the guest lays its link, the host reads its
/// layout, then the guest boots and may open them (@xpute/runtime-web
/// worker.ts `enter`). Where an interface of `imports` (the bindings'
/// `IMPORTS`) is implemented by nothing mounted, under its named handle or by
/// an opener of its resources (xpute-link `Side::implements`), or one of
/// `over`, the interfaces the UI implements that hold a function without
/// `async` (the bindings' `CALLS`), the link traps, naming each, and the
/// guest is never entered. False where the link trapped, whichever side
/// failed.
pub(crate) fn init(imports: &[(&str, Handle)], over: &[Handle]) -> bool {
    let s = side();
    let unimplemented: Vec<&str> = imports
        .iter()
        .filter(|&&(_, h)| !s.implements(h))
        .map(|&(name, _)| name)
        .collect();
    if !unimplemented.is_empty() {
        trap(&format!(
            "no implementation of {}",
            unimplemented.join(", ")
        ));
        return false;
    }
    let mut called: Vec<String> = over
        .iter()
        .map(|&h| {
            imports
                .iter()
                .find(|&&(_, i)| i == h)
                .map_or(h.to_string(), |&(name, _)| name.to_owned())
        })
        .collect();
    called.sort();
    called.dedup();
    if !called.is_empty() {
        trap(&format!(
            "a function without async is implemented by the UI: {}",
            called.join(", ")
        ));
        return false;
    }
    // SAFETY: the guest's export, called once, before any other.
    let at = s.crossed(|| unsafe { link_init() });
    if at == TRAP_AT {
        trap(GUEST_TRAPPED);
        return false;
    }
    s.set_layout(Layout::at(at));
    // SAFETY: the guest's export, called once, right after `link_init`.
    if s.crossed(|| unsafe { link_boot() }) == TRAP {
        trap(GUEST_TRAPPED);
    }
    !s.trapped()
}

/// The guest's import: the frame it laid at `at`, taken by the host's side
/// of the link on this thread, as the web's `link_out` is (@xpute/runtime-web
/// worker.ts `instantiate`). A panic of the host's while it answers, which its
/// hook has reported, unwinds no further: the link is trapped, and the
/// guest's call is answered ENOTRECOVERABLE. Once trapped, a frame is not
/// run: EPIPE, as `cross_in` answers.
#[unsafe(no_mangle)]
pub extern "C" fn link_out(at: u64) -> i32 {
    if side().trapped() {
        return Errno::EPIPE as i32;
    }
    std::panic::catch_unwind(|| side().recv(at)).unwrap_or_else(|_| {
        trap(HOST_STOPPED);
        Errno::ENOTRECOVERABLE as i32
    })
}
