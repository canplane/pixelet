// pxl-core/link/export/perf.rs

//! The perf interface's methods, which drive the profiler. A read answers
//! the words it hands over as its return data, so the client never reads the
//! core's memory.

use super::applied;
use crate::abi::types::ErrorCode;
use crate::domain::setting::settings;
use crate::perf::read;
use crate::perf::record;
use crate::perf::frame::{FrameSample, WorstFrame};
use crate::perf::record::{perf_frames, perf_strtab, perf_tasks, perf_worst, STRTAB_CAP};
use crate::spec::perf::{limit, now, stat, RING_NAMES};
use crate::perf::task::TaskRecord;
use crate::perf::window::perf_window_start;
use crate::perf::window::perf_window_stop;
use std::cell::RefCell;

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
    applied().map(Ok)
}

pub(super) fn stop() -> Result<(), Errno> {
    perf_window_stop();
    applied()
}

/// Answers STAT's words over `window_ms` and NOW's (perf.json5).
pub(super) fn stats(window_ms: Option<f64>) -> Result<Vec<u8>, Errno> {
    Ok(read_timed(|| bytes_of(read::perf_read(window_ms))))
}

/// Each ring's records, in RING_NAMES' order: frames, tasks, worst, and the
/// bytes a record takes.
const RINGS: [(fn() -> &'static [u8], usize); RING_NAMES.len()] = [
    (|| bytes_of(perf_frames()), size_of::<FrameSample>()),
    (|| bytes_of(perf_tasks()), size_of::<TaskRecord>()),
    (|| bytes_of(perf_worst()), size_of::<WorstFrame>()),
];

// The string table and the statistics are read whole, each in one page.
const _: () = assert!(STRTAB_CAP <= limit::PAGE_BYTES as usize);
const _: () = assert!(8 * (stat::WORDS + now::WORDS) <= limit::PAGE_BYTES as usize);

thread_local! {
    /// The ring being read, by its number, as it stood at its first page, so
    /// that the records landing between pages move none of its pages. It goes
    /// with its last page, the one that comes short.
    static READING: RefCell<Option<(u32, Vec<u8>)>> = const { RefCell::new(None) };
}

/// Answers up to `cnt` records of ring `ring` from its record `from`, no more
/// than a page holds (limit::PAGE_BYTES), as the ring stood at its first page,
/// the one from record 0.
pub(super) fn ring(ring: u32, from: u32, cnt: u32) -> Result<Vec<u8>, Errno> {
    let &(read, record) = RINGS.get(ring as usize).ok_or(Errno::EINVAL)?;
    if from == 0 {
        READING.set(Some((ring, read_timed(read))));
    }
    READING.with_borrow_mut(|held| {
        let Some((_, b)) = held.as_ref().filter(|(r, _)| *r == ring) else {
            return Err(Errno::EINVAL);
        };
        let page = limit::PAGE_BYTES as usize / record;
        let n = b.len() / record;
        let from = (from as usize).min(n);
        let to = from + (cnt as usize).min(page).min(n - from);
        let out = b[from * record..to * record].to_vec();
        if to - from < page {
            *held = None;
        }
        Ok(out)
    })
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
