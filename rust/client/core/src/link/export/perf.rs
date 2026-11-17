// pxl-core/link/export/perf.rs

//! The perf interface's methods, which drive the profiler. A read answers
//! the words it hands over as its return data, so the client never reads the
//! core's memory.

use crate::abi::types::ErrorCode;
use crate::domain::setting::settings;
use crate::perf::frame::{FrameSample, WorstFrame};
use crate::perf::read;
use crate::perf::record;
use crate::perf::record::{STRTAB_CAP, perf_frames, perf_strtab, perf_tasks, perf_worst};
use crate::perf::task::TaskRecord;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use crate::spec::perf::{TABLE_NAMES, limit, now, stat};
use pxl_global::Global;

use xpute_guest::clock::now;
use xpute_kit::status::errno::Errno;

/// The profiler records only while its setting is on.
pub(super) fn start() -> Result<Result<(), ErrorCode>, Errno> {
    if !settings().perf {
        return Ok(Err(ErrorCode::NotPermitted));
    }
    if !perf_window_start() {
        return Ok(Err(ErrorCode::InsufficientMemory));
    }
    Ok(Ok(()))
}

pub(super) fn stop() -> Result<(), Errno> {
    perf_window_stop();
    Ok(())
}

/// Answers STAT's words over `window_ms` and NOW's (perf.json5).
pub(super) fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
    Ok(read_timed(|| bytes_of(read::perf_read(window_ms))))
}

/// A table's records as bytes, and the bytes a record takes.
type Table = (fn() -> &'static [u8], usize);

/// Each table, in `TABLE_NAMES`' order: frames, tasks, worst.
const TABLES: [Table; TABLE_NAMES.len()] = [
    (|| bytes_of(perf_frames()), size_of::<FrameSample>()),
    (|| bytes_of(perf_tasks()), size_of::<TaskRecord>()),
    (|| bytes_of(perf_worst()), size_of::<WorstFrame>()),
];

// The string table and the statistics are read whole, each in one page.
const _: () = assert!(STRTAB_CAP <= limit::PAGE_SZ as usize);
const _: () = assert!(8 * (stat::WORDS + now::WORDS) <= limit::PAGE_SZ as usize);

/// The table being read, by its number, as it stood at its first page, so that
/// the records landing between pages move none of its pages. It goes with its
/// last page, the one that comes short.
static READING: Global<Option<(u32, Vec<u8>)>> = Global::new();

/// Answers the records of table `table` from its record `from`, as many as a
/// page holds (`limit::PAGE_SZ`), as the table stood at its first page, the
/// one from record 0.
pub(super) fn table(table: u32, from: u32) -> Result<Vec<u8>, Errno> {
    let &(read, record) = TABLES.get(table as usize).ok_or(Errno::EINVAL)?;
    if from == 0 {
        READING.set(Some((table, read_timed(read))));
    }
    let reading = READING.get(|| None);
    let Some((_, b)) = reading.as_ref().filter(|(t, _)| *t == table) else {
        return Err(Errno::EINVAL);
    };
    let page = limit::PAGE_SZ as usize / record;
    let n = b.len() / record;
    let from = (from as usize).min(n);
    let to = from + page.min(n - from);
    let out = b[from * record..to * record].to_vec();
    if to - from < page {
        *reading = None;
    }
    Ok(out)
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
