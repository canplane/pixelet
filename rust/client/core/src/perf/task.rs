// pxl-core/perf/task.rs

//! A task's row in the profiler's columns: what ran, when, and how it ended.
//! A row is made from an attempt the request scheduler ended
//! (`TaskRecord::of`), when the door takes the scheduler's end event (door.rs
//! `run`).

use core::fmt::Write;
use core::mem::{offset_of, size_of};

use xpute_core::io::fs::Token;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::perf::spec::TASK;
use crate::perf::word::{words, AsWords};
use crate::sys::io::demand::{PhaseList, Stream, PHASES_MAX};
use crate::sys::io::failure::Exit;
use crate::sys::io::file::{self, Text, KEY_BYTES};
use crate::sys::io::request::Attempt;

/// Room for a path's name as text, rounded up to whole words: a quadkey, or
/// a u32 in decimal.
const TASK_PATH_BYTES: usize = 3 * 8;
const _: () = assert!(KEY_BYTES <= TASK_PATH_BYTES);
const _: () = assert!((u32::MAX.ilog10() as usize) < TASK_PATH_BYTES);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRecord {
    pub stream: f64,
    /// The name of the path the task opened, as text, zero past its end.
    pub path: [u8; TASK_PATH_BYTES],
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub outcome: f64,
    pub fail: f64,
    pub detail: f64,
    /// Where the failure was raised: the file name's address and length, and
    /// the line. All are 0 when the runtime raised it.
    pub at_file: f64,
    pub at_file_bytes: f64,
    pub at_line: f64,
    pub phase_cnt: f64,
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_vals: [f64; PHASES_MAX],
}

words!(TaskRecord, TASK::WORDS,
    stream => TASK::stream, path => TASK::path, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_cnt => TASK::phase_cnt, phase_ids => TASK::phase_ids, phase_vals => TASK::phase_vals,
);

/// The name of the file `stream`'s want `id` opens, its path's last token,
/// as text: its quadkey, or its number in decimal. It is zero past its end,
/// and all zero for a want that opens no file.
fn path_bytes(stream: Stream, id: u64) -> [u8; TASK_PATH_BYTES] {
    let mut out = Text::EMPTY;
    file::with_path(stream, id, |path| match path.last() {
        Some(Token::Int(n)) => write!(out, "{n}"),
        Some(Token::Str(key)) => out.write_str(key),
        None => Ok(()),
    })
    .transpose()
    .or_bug(Errno::EOVERFLOW);
    out.bytes
}

impl TaskRecord {
    /// The row of `a`, which ended at `at` with `exit`, and the phase timings
    /// its run reported. `name_at` gives a failure's file name its offset in
    /// the profiler's string table, or none where the table has no room.
    pub fn of(
        a: &Attempt,
        at: f64,
        exit: Exit,
        phases: Option<PhaseList>,
        name_at: &mut dyn FnMut(&'static str) -> Option<u32>,
    ) -> TaskRecord {
        let mut row = TaskRecord {
            stream: a.stream as u32 as f64,
            path: path_bytes(a.stream, a.id),
            priority: a.rank.class as f64 + a.rank.dist,
            enqueue_ms: a.enqueue_ms,
            start_ms: a.start_ms,
            end_ms: at,
            outcome: exit.outcome() as u32 as f64,
            ..TaskRecord::default()
        };
        match exit {
            Exit::Failed(fail) => {
                row.fail = f64::from(fail.errnum as i32);
                row.detail = fail.detail as f64;
            }
            Exit::Refused(_) => row.fail = f64::from(Errno::ENOMEM as i32),
            Exit::Ok | Exit::Aborted => {}
        }
        if let Some(at) = match exit {
            Exit::Failed(fail) => fail.at,
            Exit::Ok | Exit::Aborted | Exit::Refused(_) => None,
        } {
            if let Some(off) = name_at(at.file()) {
                row.at_file = f64::from(off);
                row.at_file_bytes = at.file().len() as f64;
            }
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
