// pxl-core/link/export/perf.rs

//! The perf interface's methods, which drive the profiler. A read answers
//! the words it hands over as its return data, so the client never reads the
//! core's memory. Each read is one call that keeps nothing between calls: it
//! copies what it answers, one page at most (`limit::PAGE_SZ`), onto the
//! core's heap, and the copy is freed once the reply is laid.

use crate::abi::perf::Rows;
use crate::abi::types::ErrorCode;
use crate::domain::setting::settings;
use crate::perf::frame::FrameSample;
use crate::perf::read;
use crate::perf::record;
use crate::perf::record::{
    STRTAB_CAP, perf_frame_seq, perf_frames, perf_strtab, perf_task_seq, perf_tasks, perf_worst,
};
use crate::perf::task::TaskRecord;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use crate::spec::perf::{TABLE_NAMES, limit, now, stat};

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

/// A table: its rows as bytes, the sequence past its newest row, and the
/// bytes a row takes.
type Table = (fn() -> &'static [u8], fn() -> u64, usize);

/// Each table, in `TABLE_NAMES`' order: frames, tasks.
const TABLES: [Table; TABLE_NAMES.len()] = [
    (
        || bytes_of(perf_frames()),
        perf_frame_seq,
        size_of::<FrameSample>(),
    ),
    (
        || bytes_of(perf_tasks()),
        perf_task_seq,
        size_of::<TaskRecord>(),
    ),
];

// The string table and the statistics are read whole, each in one page, as
// the worst frames are (perf/record.rs `WORST_CAP`).
const _: () = assert!(STRTAB_CAP <= limit::PAGE_SZ as usize);
const _: () = assert!(8 * (stat::WORDS + now::WORDS) <= limit::PAGE_SZ as usize);

/// Answers the rows of table `table` from the sequence `from` on, `cnt` at
/// most and as many as a page holds (`limit::PAGE_SZ`), with the sequence of
/// the first of them and the one past the table's newest row. A `from` past
/// the table's end names no row and asks for its newest `cnt` rows, so it
/// counts `cnt` back from the end. The first is `from` held to the rows the
/// table has: its oldest where `from` is a row it dropped.
pub(super) fn table(table: u32, from: u64, cnt: u32) -> Result<Rows, Errno> {
    let &(rows, end, row_sz) = TABLES.get(table as usize).ok_or(Errno::EINVAL)?;
    let (rows, end) = (rows(), end());
    let from = match from > end {
        true => end.saturating_sub(u64::from(cnt)),
        false => from,
    };
    let first = from.max(end - (rows.len() / row_sz) as u64);
    let n = ((end - first) as usize)
        .min(cnt as usize)
        .min(limit::PAGE_SZ as usize / row_sz);
    let at = rows.len() - (end - first) as usize * row_sz;
    let bytes = read_timed(|| &rows[at..at + n * row_sz]);
    Ok(Rows { first, end, bytes })
}

/// Answers the worst frames, longest first.
pub(super) fn worst() -> Result<Vec<u8>, Errno> {
    Ok(read_timed(|| bytes_of(perf_worst())))
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
