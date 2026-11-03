// xpute-guest/stop.rs

//! How a guest stops: a broken invariant (`bug!`) or a panic, published to
//! the host as the turn's one record (what `install` is given, the guest's
//! `Sys::abort`) and told to the hook of a form whose guest shares its process
//! with the host, where a panic would otherwise take the host with it.

use core::panic::Location;

use xpute_kit::status::bug::{self, Report};
use xpute_kit::status::errno::Errno;

use crate::global::{off_marked_thread, Global};

/// Numbers only: formatting is the hook's.
pub enum Stop<'a> {
    Bug {
        errnum: Errno,
        at: &'a Location<'a>,
        a: u64,
        b: u64,
    },
    Panic(&'a std::panic::PanicHookInfo<'a>),
    /// The guest has stopped, told last: the form ends its thread here (xpute-host
    /// native `stopped`), or the process goes with it.
    End,
}

/// Where a form sharing its process with the guest says why, a panic's text not
/// being in the record's numbers, and keeps the process alive.
pub type PanicHook = fn(Stop<'_>);

static ABORT: Global<Option<Report>> = Global::new();

static PANIC_HOOK: Global<Option<PanicHook>> = Global::new();

static STOPPED: Global<bool> = Global::new();

/// Installs the bug reporter and std's panic hook, once at boot: `abort`
/// publishes the record, `panic_hook` is the form's, none where the host
/// outlives the guest (a page). Both are process-wide, and where the guest is
/// linked into a native program the program's other threads fail through
/// them too: such a panic is not the guest's, and is told by the
/// `PanicHook` where there is one, else by the std hook this one replaced; a
/// bug there is not reported, and is told as the panic it then is
/// (`global::off_marked_thread`).
pub fn install(abort: Report, panic_hook: Option<PanicHook>) {
    ABORT.set(Some(abort));
    PANIC_HOOK.set(panic_hook);
    bug::set_report(|errnum, at, a, b| {
        if !off_marked_thread() {
            stopped(errnum, at, a, b);
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
        if !is_stopped() {
            say(Stop::Panic(info));
            if let Some(at) = info.location() {
                stopped(Errno::ENOTRECOVERABLE, at, 0, 0);
            }
        }
        say(Stop::End);
    }));
}

/// Told to the hook as well as the host.
fn stopped(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    STOPPED.set(true);
    say(Stop::Bug { errnum, at, a, b });
    if let Some(abort) = *ABORT.get(|| None) {
        abort(errnum, at, a, b);
    }
}

/// Past its report a bug panics where it cannot trap, and that panic is its
/// way out.
fn is_stopped() -> bool {
    *STOPPED.get(|| false)
}

fn say(stop: Stop<'_>) {
    if let Some(say) = *PANIC_HOOK.get(|| None) {
        say(stop);
    }
}
