// xpute-core/stop.rs

//! How a core stops. A broken invariant (`bug!`) or a panic is posted to the
//! runtime as the turn's only frame, by the reporter `install` is given (the
//! core's `Door::abort`). Where the core shares its process with the runtime,
//! the stop is also told to the form's hook, since a panic there would
//! otherwise take the runtime down with it.

use core::panic::Location;

use xpute_kit::status::bug::{self, Report};
use xpute_kit::status::errno::Errno;

use crate::global::{off_marked_thread, Global};

/// A stop holds numbers only; the hook does any formatting.
pub enum Stop<'a> {
    Bug {
        errnum: Errno,
        at: &'a Location<'a>,
        a: u64,
        b: u64,
    },
    Panic(&'a std::panic::PanicHookInfo<'a>),
    /// The core has stopped. This is told last, inside the panic hook. The
    /// form then ends the core's thread: outside the hook once the panic has
    /// unwound out of the turn, or inside it where a panic aborts
    /// (xpute-runtime native `stopped`). Without a form, the process ends
    /// with the core.
    End,
}

/// The hook through which a form that shares its process with the core says
/// why the core stopped, and keeps the process alive. The runtime's record
/// holds numbers only, so a panic's text reaches no one else.
pub type PanicHook = fn(Stop<'_>);

static REPORT: Global<Option<Report>> = Global::new();

static PANIC_HOOK: Global<Option<PanicHook>> = Global::new();

static ABORTED: Global<bool> = Global::new();

/// Installs the bug reporter and std's panic hook, once at boot. `report`
/// publishes the record; `panic_hook` is the form's, and is none where the
/// runtime outlives the core (a page).
///
/// Both hooks are process-wide. Where the core is linked into a native
/// program, the program's other threads fail through them too
/// (`global::off_marked_thread`). Such a panic is not the core's: it is told
/// to the `PanicHook` if there is one, else to the std hook this one
/// replaced. A bug on such a thread is not reported; it is told as the panic
/// it then becomes.
pub fn install(report: Report, panic_hook: Option<PanicHook>) {
    REPORT.set(Some(report));
    PANIC_HOOK.set(panic_hook);
    bug::set_report(|errnum, at, a, b| {
        if !off_marked_thread() {
            abort(errnum, at, a, b);
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
        if !is_aborted() {
            say(Stop::Panic(info));
            if let Some(at) = info.location() {
                abort(Errno::ENOTRECOVERABLE, at, 0, 0);
            }
        }
        say(Stop::End);
    }));
}

/// Tells a bug to the form's hook as well as to the runtime.
fn abort(errnum: Errno, at: &Location<'_>, a: u64, b: u64) {
    ABORTED.set(true);
    say(Stop::Bug { errnum, at, a, b });
    if let Some(report) = *REPORT.get(|| None) {
        report(errnum, at, a, b);
    }
}

/// Whether a bug has already been reported. After its report, a bug panics
/// on a target where it cannot trap, and the panic hook must not report that
/// panic again.
fn is_aborted() -> bool {
    *ABORTED.get(|| false)
}

fn say(stop: Stop<'_>) {
    if let Some(say) = *PANIC_HOOK.get(|| None) {
        say(stop);
    }
}
