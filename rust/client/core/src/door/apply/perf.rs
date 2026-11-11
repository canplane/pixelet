// pxl-core/door/apply/perf.rs

//! The PERF major's commands, which drive the profiler. A read's reply names
//! the words it hands over, which go with the reply's frame, so the client
//! never reads the core's memory after the turn that wrote it.

use super::State;
use super::{ask_turn, refuse_unless};
use crate::domain::setting::store::settings;
use crate::door::cmd as abi;
use crate::perf::read;
use crate::perf::record;
use crate::perf::record::{perf_frames, perf_strtab, perf_tasks, perf_worst};
use crate::perf::spec::RING_NAMES;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use xpute_core::clock::now;
use xpute_core::io::door::{Outcome, Reply};
use xpute_kit::status::errno::Errno;
use xpute_kit::status::error::MarshalError;

/// The profiler records only while its setting is on.
pub(super) fn start(s: &mut State) -> Outcome {
    refuse_unless(settings().perf, Errno::EPERM)?;
    refuse_unless(perf_window_start(), Errno::ENOMEM)?;
    ask_turn(s)
}

pub(super) fn stop(s: &mut State) -> Outcome {
    perf_window_stop();
    ask_turn(s)
}

/// Replies with STAT's words over `window_ms` and NOW's (perf.json).
pub(super) fn stats(c: abi::PerfStats) -> Outcome {
    Ok(Some(read_timed(|| bytes_of(read::perf_read(c.window_ms)))))
}

/// Each ring's records, in RING_NAMES' order: frames, tasks, worst.
const RINGS: [fn() -> &'static [u8]; RING_NAMES] = [
    || bytes_of(perf_frames()),
    || bytes_of(perf_tasks()),
    || bytes_of(perf_worst()),
];

/// Replies with the records of ring `c.ring`.
pub(super) fn rings(c: abi::PerfRings) -> Outcome {
    let ring = RINGS
        .get(c.ring as usize)
        .ok_or_else(|| MarshalError::new(Errno::EINVAL))?;
    Ok(Some(read_timed(ring)))
}

/// Replies with the string table the tasks' `at_file` names point into.
pub(super) fn strtab() -> Outcome {
    Ok(Some(read_timed(perf_strtab)))
}

/// Runs a read of the profiler and records its time as the profiler's own
/// cost to the interval (perf/record.rs `perf_record_read`).
fn read_timed(read: impl FnOnce() -> &'static [u8]) -> Reply {
    let at = now();
    let named = read();
    record::perf_record_read(now() - at);
    Reply { val: 0, named }
}

/// The bytes of `words`, records of plain words in the core's memory.
fn bytes_of<T: Copy>(words: &'static [T]) -> &'static [u8] {
    // SAFETY: the same memory, read as bytes, for as long as `words` lives.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), size_of_val(words)) }
}
