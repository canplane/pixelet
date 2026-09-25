// xpute-core/status/bug.rs
// (no pair: a script's thrown error carries its own stack)

//! A broken invariant, said as a number and a place: the errno of what did
//! not hold, where in the source it did not, and at most two numbers the
//! place cannot give back — a count, an index. Nothing else, and no
//! sentence: the place names the line that failed, and the line says the
//! rest.
//!
//! That is how a kernel reports one. A Windows bug check is a code and four
//! numbers; Linux's `BUG()` is a trap and a table from its address to a file
//! and a line; neither ships the text that explains it, which lives beside
//! the build instead. The failure is fatal all the same (`error.rs`'s
//! policy): what reports it is installed by whoever assembles the module
//! (`set_report`), and the module stops after.

use core::panic::Location;
use std::sync::OnceLock;

use crate::status::errno::Errno;

/// Where a report goes before the module stops.
pub type Report = fn(errno: Errno, at: &Location<'_>, a: u64, b: u64);

static REPORT: OnceLock<Report> = OnceLock::new();

/// Installs where a report goes; the first installed stands.
pub fn set_report(report: Report) {
    let _ = REPORT.set(report);
}

/// Reports `errno` at the caller's place, with `a` and `b`, and stops.
#[track_caller]
#[cold]
#[inline(never)]
pub fn bug(errno: Errno, a: u64, b: u64) -> ! {
    let at = Location::caller();
    if let Some(report) = REPORT.get() {
        report(errno, at, a, b);
    }
    // The module traps: the report is all that is said, and the panic
    // machinery, its formatting with it, is not reached.
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    panic!("{errno:?} at {at} ({a}, {b})");
}

/// A value an invariant says is there.
pub trait OrBug<T> {
    /// The value, or `errno` reported at the caller's place.
    #[track_caller]
    fn or_bug(self, errno: Errno) -> T;
}

impl<T> OrBug<T> for Option<T> {
    #[track_caller]
    fn or_bug(self, errno: Errno) -> T {
        match self {
            Some(v) => v,
            None => bug(errno, 0, 0),
        }
    }
}

impl<T, E> OrBug<T> for Result<T, E> {
    #[track_caller]
    fn or_bug(self, errno: Errno) -> T {
        match self {
            Ok(v) => v,
            Err(_) => bug(errno, 0, 0),
        }
    }
}

/// `bug!(ENOSPC)`, `bug!(ENOSPC, n)`, `bug!(ENOSPC, n, cap)`: an errno by
/// its name and at most two numbers, reported where it is written.
#[macro_export]
macro_rules! bug {
    ($errno:ident) => {
        $crate::status::bug::bug($crate::status::errno::Errno::$errno, 0, 0)
    };
    ($errno:ident, $a:expr) => {
        $crate::status::bug::bug($crate::status::errno::Errno::$errno, ($a) as u64, 0)
    };
    ($errno:ident, $a:expr, $b:expr) => {
        $crate::status::bug::bug($crate::status::errno::Errno::$errno, ($a) as u64, ($b) as u64)
    };
}

/// `ensure!(cond, ENOSPC, n)`: `bug!` unless `cond` holds.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $errno:ident $(, $x:expr)* $(,)?) => {
        if !($cond) {
            $crate::bug!($errno $(, $x)*)
        }
    };
}
