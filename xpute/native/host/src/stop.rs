// xpute-native-host/stop.rs

//! The stop the host serves the guest (wit/stop/stop.wit): the guest's report
//! of a broken invariant, logged and handed to the platform, before the guest
//! stops (stop.ts). The host's own failures take the same path, and then trap
//! the link (lib.rs `hook`), as a page's worker that dies is over; a guest's
//! trap is told on its own (`trapped`), as a page's stop and trap are two
//! messages.

use std::rc::Rc;
use std::sync::OnceLock;

use xpute_kit::status::errno::Errno;

use crate::abi;

/// The guest's report.
pub struct Stop {
    pub errnum: i32,
    pub file: String,
    pub line: u32,
    pub n1: u64,
    pub n2: u64,
}

struct Report(fn(Stop));

impl abi::stop::stop::Stop for Report {
    fn stopped(&self, errnum: i32, file: String, line: u32, n1: u64, n2: u64) -> Result<(), Errno> {
        crate::log(&format!(
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
/// failures reach, and its hearing of a trap.
static REPORT: OnceLock<Hears> = OnceLock::new();

/// The platform's report of a stop, and of a trap.
type Hears = (fn(Stop), fn());

/// Mounts the stop, `report` hearing a stop and `trapped` a trap.
pub fn serve(report: fn(Stop), trapped: fn()) {
    let _ = REPORT.set((report, trapped));
    abi::stop::stop::mount(Rc::new(Report(report)));
}

/// Tells the platform the guest trapped, as a page's worker tells its trap.
pub(crate) fn trapped() {
    crate::log("[guest] trapped");
    if let Some((_, trapped)) = REPORT.get() {
        trapped();
    }
}

/// Logs a failure of the host's own and hands it to the platform: from
/// any thread, without the link.
pub(crate) fn report_own(errnum: i32, file: &str, line: u32) {
    crate::log(&format!("[host] stopped: {errnum} at {file}:{line}"));
    if let Some((report, _)) = REPORT.get() {
        report(Stop {
            errnum,
            file: file.to_owned(),
            line,
            n1: 0,
            n2: 0,
        });
    }
}
