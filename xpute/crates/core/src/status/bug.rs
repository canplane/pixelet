// xpute-core/status/bug.rs
// (no pair: a script's thrown error carries its own stack)

//! A broken invariant: an errno, the source place, and at most two numbers,
//! reported through `set_report`; then the module stops.

use core::panic::Location;
use std::sync::OnceLock;

use crate::status::errno::Errno;

pub type Report = fn(errno: Errno, at: &Location<'_>, a: u64, b: u64);

static REPORT: OnceLock<Report> = OnceLock::new();

/// The first installed stands.
pub fn set_report(report: Report) {
    let _ = REPORT.set(report);
}

#[track_caller]
#[cold]
#[inline(never)]
pub fn bug(errno: Errno, a: u64, b: u64) -> ! {
    let at = Location::caller();
    if let Some(report) = REPORT.get() {
        report(errno, at, a, b);
    }
    // The report is all that is said; the trap skips panic formatting.
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    panic!("{errno:?} at {at} ({a}, {b})");
}

pub trait OrBug<T> {
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

/// `bug!(ENOSPC)`, `bug!(ENOSPC, n)`, `bug!(ENOSPC, n, cap)`.
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

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $errno:ident $(, $x:expr)* $(,)?) => {
        if !($cond) {
            $crate::bug!($errno $(, $x)*)
        }
    };
}
