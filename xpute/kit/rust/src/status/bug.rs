// xpute-kit/status/bug.rs

//! A broken invariant stops the module. A bug is an errno, its place in the
//! source, at most two numbers and an optional message. It is handed to the
//! report that `set_report` installed, and then the module stops: a
//! WebAssembly module traps, and a native build panics. The message is a
//! static string, never one built at the bug: a full heap is itself a bug, so
//! reporting one allocates nothing, and what varies goes in the two numbers.

use core::panic::Location;
use std::sync::OnceLock;

use crate::status::errno::Errno;

pub type Report = fn(errnum: Errno, at: &Location<'_>, a: u64, b: u64, msg: Option<&'static str>);

static REPORT: OnceLock<Report> = OnceLock::new();

/// Installs the function a bug is reported to. Only the first call takes
/// effect.
pub fn set_report(report: Report) {
    let _ = REPORT.set(report);
}

/// What a native build's bug panics with: the bug itself, so that a panic
/// hook tells it from any other panic, and does not report it again where
/// the report took it.
#[derive(Debug)]
pub struct XputeBug {
    pub errnum: Errno,
    pub file: &'static str,
    pub line: u32,
    pub a: u64,
    pub b: u64,
    pub msg: Option<&'static str>,
}

#[track_caller]
#[cold]
#[inline(never)]
pub fn bug(errnum: Errno, a: u64, b: u64) -> ! {
    stop(errnum, Location::caller(), a, b, None)
}

/// The bug `errnum`, with what a person reads of it.
#[track_caller]
#[cold]
#[inline(never)]
pub fn bug_with_msg(errnum: Errno, a: u64, b: u64, msg: &'static str) -> ! {
    stop(errnum, Location::caller(), a, b, Some(msg))
}

/// Reports the bug made at `at` and stops the module.
fn stop(
    errnum: Errno,
    at: &'static Location<'static>,
    a: u64,
    b: u64,
    msg: Option<&'static str>,
) -> ! {
    if let Some(report) = REPORT.get() {
        report(errnum, at, a, b, msg);
    }
    // The report is all the host is told, so a WebAssembly module traps
    // at once and spends nothing on formatting a panic message.
    #[cfg(target_arch = "wasm32")]
    core::arch::wasm32::unreachable();
    #[cfg(not(target_arch = "wasm32"))]
    std::panic::panic_any(XputeBug {
        errnum,
        file: at.file(),
        line: at.line(),
        a,
        b,
        msg,
    });
}

pub trait OrBug<T> {
    #[track_caller]
    fn or_bug(self, errnum: Errno) -> T;
}

impl<T> OrBug<T> for Option<T> {
    #[track_caller]
    fn or_bug(self, errnum: Errno) -> T {
        match self {
            Some(v) => v,
            None => bug(errnum, 0, 0),
        }
    }
}

impl<T, E> OrBug<T> for Result<T, E> {
    #[track_caller]
    fn or_bug(self, errnum: Errno) -> T {
        match self {
            Ok(v) => v,
            Err(_) => bug(errnum, 0, 0),
        }
    }
}

/// Reports a bug with an errno, named by its constant, and up to two numbers:
/// `bug!(ENOSPC)`, `bug!(ENOSPC, n)` or `bug!(ENOSPC, n, cap)`. A message
/// follows a `;`, which no number does: `bug!(ENOSPC, n, cap; "the heap is full")`.
#[macro_export]
macro_rules! bug {
    ($errnum:ident; $msg:expr) => {
        $crate::bug!($errnum, 0, 0; $msg)
    };
    ($errnum:ident, $a:expr; $msg:expr) => {
        $crate::bug!($errnum, $a, 0; $msg)
    };
    ($errnum:ident, $a:expr, $b:expr; $msg:expr) => {
        $crate::status::bug::bug_with_msg(
            $crate::status::errno::Errno::$errnum,
            ($a) as u64,
            ($b) as u64,
            $msg,
        )
    };
    ($errnum:ident) => {
        $crate::status::bug::bug($crate::status::errno::Errno::$errnum, 0, 0)
    };
    ($errnum:ident, $a:expr) => {
        $crate::status::bug::bug($crate::status::errno::Errno::$errnum, ($a) as u64, 0)
    };
    ($errnum:ident, $a:expr, $b:expr) => {
        $crate::status::bug::bug(
            $crate::status::errno::Errno::$errnum,
            ($a) as u64,
            ($b) as u64,
        )
    };
}

#[macro_export]
macro_rules! ensure {
    ($cond:expr, $errnum:ident $(, $x:expr)* $(,)?) => {
        if !($cond) {
            $crate::bug!($errnum $(, $x)*)
        }
    };
}
