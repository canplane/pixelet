// pxl-kernel/sys/stop.rs

//! How the kernel stops: a broken invariant (`bug!`) or a panic, published to
//! the host (sys/io/host.rs `abort`) and told to the bracket's panic hook.

use core::panic::Location;

use crate::sys::io::host;
use xpute_guest::global::{off_marked_thread, Global};
use xpute_kit::status::bug;
use xpute_kit::status::errno::Errno;

/// Numbers only: formatting is the bracket's, so a module that reports
/// nothing links none of it.
pub enum Stop<'a> {
    Bug {
        errnum: Errno,
        at: &'a Location<'a>,
        a: u64,
        b: u64,
    },
    Panic(&'a std::panic::PanicHookInfo<'a>),
}

pub type PanicHook = fn(Stop<'_>);

static PANIC_HOOK: Global<Option<PanicHook>> = Global::new();

static STOPPED: Global<bool> = Global::new();

/// Installs the bug reporter and std's panic hook, once at boot. Both are
/// process-wide, and on a device the app's other threads fail through them
/// too: such a panic is not the kernel's, and is told by the bracket's
/// `PanicHook` where it has one, else by the std hook this one replaced,
/// never by both; a bug there is not reported, and is told as the panic it
/// then is (xpute_guest::global `off_marked_thread`).
pub fn install(panic_hook: Option<PanicHook>) {
    PANIC_HOOK.set(panic_hook);
    bug::set_report(|errnum, at, a, b| {
        if !off_marked_thread() {
            bug(errnum, at, a, b);
        }
    });
    let theirs = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if off_marked_thread() {
            match panic_hook {
                Some(say) => say(Stop::Panic(info)),
                None => theirs(info),
            }
            return;
        }
        if is_stopped() {
            return;
        }
        say(Stop::Panic(info));
        if let Some(at) = info.location() {
            bug(Errno::ENOTRECOVERABLE, at, 0, 0);
        }
    }));
}

/// Told to the bracket as well as the host: where the process dies with the
/// kernel, no host is left to run the record.
fn bug(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    STOPPED.set(true);
    say(Stop::Bug { errnum, at, a, b });
    host::abort(errnum, at, a, b);
}

/// Past its report a bug panics where it cannot trap, and that panic is its
/// way out, not a second fault.
fn is_stopped() -> bool {
    *STOPPED.get(|| false)
}

fn say(stop: Stop<'_>) {
    if let Some(say) = *PANIC_HOOK.get(|| None) {
        say(stop);
    }
}
