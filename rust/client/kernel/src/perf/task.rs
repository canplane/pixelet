// pxl-kernel/perf/task.rs

//! A task's row in the profiler's columns: what ran, when, and what it came
//! to, made of an attempt the request scheduler ended (`TaskRow::of`), which
//! the top takes of it (door.rs `run`).

use core::mem::{offset_of, size_of};

use fnv::FnvHashMap;
use xpute_core::global::Global;
use xpute_core::mem::off_of_slice;
use xpute_kit::status::errno::Errno;

use crate::perf::spec::TASK;
use crate::perf::word::{words, Words};
use crate::sys::io::demand::{Phases, PHASES_MAX};
use crate::sys::io::failure::Ended;
use crate::sys::io::file::{path_of, PATH_BYTES};
use crate::sys::io::request::Attempt;

/// A path's bytes in whole words.
const TASK_PATH_BYTES: usize = 3 * 8;
const _: () = assert!(PATH_BYTES <= TASK_PATH_BYTES);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRow {
    pub stream: f64,
    /// The path it opened, its bytes zero past their end.
    pub path: [u8; TASK_PATH_BYTES],
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub outcome: f64,
    pub fail: f64,
    pub detail: f64,
    /// Failure location: file name address and length, and line. 0 when the runtime raised it.
    pub at_file: f64,
    pub at_file_bytes: f64,
    pub at_line: f64,
    pub phase_cnt: f64,
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_vals: [f64; PHASES_MAX],
}

words!(TaskRow, TASK::WORDS,
    stream => TASK::stream, path => TASK::path, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_cnt => TASK::phase_cnt, phase_ids => TASK::phase_ids, phase_vals => TASK::phase_vals,
);

/// `path`'s bytes as a row holds them.
fn path_bytes(path: &[u8]) -> [u8; TASK_PATH_BYTES] {
    let mut out = [0; TASK_PATH_BYTES];
    out[..path.len()].copy_from_slice(path);
    out
}

impl TaskRow {
    /// The row of `a`, ended at `at` as `ended` says, with the timings its run
    /// reported.
    pub fn of(a: &Attempt, at: f64, ended: Ended, phases: Option<Phases>) -> TaskRow {
        let mut row = TaskRow {
            stream: a.stream as u32 as f64,
            path: path_bytes(path_of(a.stream, a.id).as_bytes()),
            priority: a.rank.class as f64 + a.rank.dist,
            enqueue_ms: a.enqueue_ms,
            start_ms: a.start_ms,
            end_ms: at,
            outcome: ended.outcome() as u32 as f64,
            ..TaskRow::default()
        };
        match ended {
            Ended::Failed(fail) => {
                row.fail = f64::from(fail.errnum as i32);
                row.detail = fail.detail as f64;
            }
            Ended::Refused(_) => row.fail = f64::from(Errno::ENOMEM as i32),
            Ended::Ok | Ended::Aborted => {}
        }
        if let Some(at) = match ended {
            Ended::Failed(fail) => fail.at,
            Ended::Ok | Ended::Aborted | Ended::Refused(_) => None,
        } {
            row.at_file = off_of_slice(file_in_mem(at.file()).as_bytes()) as f64;
            row.at_file_bytes = at.file().len() as f64;
            row.at_line = at.line() as f64;
        }
        for (k, (name, val)) in phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name as u32 as f64;
            row.phase_vals[k] = val;
            row.phase_cnt = (k + 1) as f64;
        }
        row
    }
}

static FILES: Global<FnvHashMap<usize, Box<str>>> = Global::new();

/// A location's file name lies outside kernel memory on a device, so rows
/// name a heap copy; the names are few and kept for good.
fn file_in_mem(file: &'static str) -> &'static str {
    FILES
        .get(FnvHashMap::default)
        .entry(file.as_ptr().addr())
        .or_insert_with(|| file.into())
}
