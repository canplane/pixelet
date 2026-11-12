// pxl-core/door/apply/perf.rs

//! The perf interface's methods, which drive the profiler. A read answers
//! the words it hands over as its return data, so the client never reads the
//! core's memory.

use super::{applied, refuse_unless};
use crate::domain::setting::store::settings;
use crate::perf::read;
use crate::perf::record;
use crate::perf::record::{perf_frames, perf_strtab, perf_tasks, perf_worst};
use crate::perf::spec::RING_NAMES;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use xpute_core::clock::now;
use xpute_kit::status::errno::Errno;

/// The profiler records only while its setting is on.
pub(super) fn start() -> Result<(), Errno> {
    refuse_unless(settings().perf, Errno::EPERM)?;
    refuse_unless(perf_window_start(), Errno::ENOMEM)?;
    applied()
}

pub(super) fn stop() -> Result<(), Errno> {
    perf_window_stop();
    applied()
}

/// Answers STAT's words over `window_ms` and NOW's (perf.json).
pub(super) fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
    Ok(read_timed(|| bytes_of(read::perf_read(window_ms))))
}

/// Each ring's records, in RING_NAMES' order: frames, tasks, worst.
const RINGS: [fn() -> &'static [u8]; RING_NAMES] = [
    || bytes_of(perf_frames()),
    || bytes_of(perf_tasks()),
    || bytes_of(perf_worst()),
];

/// Answers the records of ring `ring`.
pub(super) fn rings(ring: u32) -> Result<Vec<u8>, Errno> {
    let ring = RINGS.get(ring as usize).ok_or(Errno::EINVAL)?;
    Ok(read_timed(ring))
}

/// Answers the string table the tasks' `at_file` names point into.
pub(super) fn strtab() -> Result<Vec<u8>, Errno> {
    Ok(read_timed(perf_strtab))
}

/// Runs a read of the profiler and records its time as the profiler's own
/// cost to the interval (perf/record.rs `perf_record_read`).
fn read_timed(read: impl FnOnce() -> &'static [u8]) -> Vec<u8> {
    let at = now();
    let words = read().to_vec();
    record::perf_record_read(now() - at);
    words
}

/// The bytes of `words`, records of plain words in the core's memory.
fn bytes_of<T: Copy>(words: &'static [T]) -> &'static [u8] {
    // SAFETY: the same memory, read as bytes, for as long as `words` lives.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), size_of_val(words)) }
}
