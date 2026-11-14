// xpute-native-host/stop.rs

//! The stop the host serves the guest (wit/stop/stop.wit): the guest's report
//! of a broken invariant, logged and handed to the platform, before the guest
//! stops (stop.ts). The host's own failures take the same path, and then trap
//! the link (`hook`), as a page's worker that dies is over; a guest's
//! trap is told by its runtime (xpute-native-runtime link.rs `trap`), as a
//! page's stop and trap are two messages.

use std::rc::Rc;
use std::sync::OnceLock;

use xpute_kit::status::bug::Bug;
use xpute_kit::status::errno::Errno;
use xpute_native_runtime::abi;
use xpute_native_runtime::log;
use xpute_native_runtime::native::Stop;
use xpute_native_runtime::{link, post_task};

struct Report(fn(Stop));

impl abi::stop::Host for Report {
    fn stopped(&self, errnum: i32, file: String, line: u32, n1: u64, n2: u64) -> Result<(), Errno> {
        log(&format!(
            "[guest] stopped: {errnum} at {file}:{line} ({n1}, {n2})"
        ));
        (self.0)(Stop {
            errnum,
            file,
            line,
            n1,
            n2,
        });
        Ok(())
    }
}

/// The platform's report, which both the guest's stop and the host's own
/// failures reach.
static REPORT: OnceLock<fn(Stop)> = OnceLock::new();

/// Mounts the stop, `report` hearing it.
pub fn serve(report: fn(Stop)) {
    let _ = REPORT.set(report);
    abi::stop::mount(Rc::new(Report(report)));
}

/// Makes a panic of the host's own, on any thread, a stop, as stop.ts's `hook`
/// makes a throw one: the link is trapped, so that no answer the dead thread
/// owed leaves the guest waiting, and where it traps just now, the failure is
/// logged and handed to the platform, with its errno, a bug's or
/// ENOTRECOVERABLE, and where it was raised, before the platform hears the
/// trap. A panic after the guest trapped is no failure of the host's. The
/// link's state is the guest thread's: a panic there is told at once, before
/// it unwinds, and one on another thread is posted to it. A guest installs its own hook over this one and hands back
/// what is not its own (xpute-guest link.rs `in_guest`). A bug panics with
/// itself, which no report outside the guest took.
pub fn hook() {
    std::panic::set_hook(Box::new(|info| {
        let (errnum, file, line) = match info.payload().downcast_ref::<Bug>() {
            Some(b) => (b.errnum as i32, b.file.to_owned(), b.line),
            None => {
                let (file, line) = info.location().map_or(("", 0), |at| (at.file(), at.line()));
                (Errno::ENOTRECOVERABLE as i32, file.to_owned(), line)
            }
        };
        let fail = move || {
            if !link::trapped() {
                report_own(errnum, &file, line);
            }
            link::trap();
        };
        if link::on_guest_thread() {
            fail();
        } else {
            post_task(Box::new(fail));
        }
    }));
}

/// Logs a failure of the host's own and hands it to the platform.
fn report_own(errnum: i32, file: &str, line: u32) {
    log(&format!("[host] stopped: {errnum} at {file}:{line}"));
    if let Some(report) = REPORT.get() {
        report(Stop {
            errnum,
            file: file.to_owned(),
            line,
            n1: 0,
            n2: 0,
        });
    }
}
