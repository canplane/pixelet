// pxl-core/perf/task.rs

//! A task's row in the profiler's task table: what ran, when, and how it ended.
//! A row is made from an attempt the request scheduler ended
//! (`TaskRecord::of`), when the link takes the scheduler's end event (link.rs
//! `end`).

use core::fmt::Write;
use core::mem::{offset_of, size_of};

use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;
use xpute_link::abi::types::Token;

use crate::perf::word::{AsWords, words};
use crate::spec::perf::task;
use crate::sys::io::demand::{PHASE_CAP, PhaseList, Source, Stream};
use crate::sys::io::failure::Exit;
use crate::sys::io::file::{self, KEY_SZ, Text};
use crate::sys::io::request::Attempt;
use crate::sys::load;

/// Room for a path's name as text, rounded up to whole words: a quadkey, or
/// a u32 in decimal.
const TASK_PATH_SZ: usize = 3 * 8;
const _: () = assert!(KEY_SZ <= TASK_PATH_SZ);
const _: () = assert!((u32::MAX.ilog10() as usize) < TASK_PATH_SZ);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TaskRecord {
    pub stream: f64,
    /// The name of the path the task opened, as text, zero past its end.
    pub path: [u8; TASK_PATH_SZ],
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub outcome: f64,
    pub fail: f64,
    pub detail: f64,
    /// Where the failure was raised: the file name's offset in the string
    /// table and its length, and the line. All are 0 when the host raised it.
    pub at_file: f64,
    pub at_file_sz: f64,
    pub at_line: f64,
    pub phase_cnt: f64,
    pub phase_ids: [f64; PHASE_CAP],
    pub phase_vals: [f64; PHASE_CAP],
}

words!(TaskRecord, task::WORDS,
    stream => task::STREAM, path => task::PATH, priority => task::PRIORITY, enqueue_ms => task::ENQUEUE_MS,
    start_ms => task::START_MS, end_ms => task::END_MS, outcome => task::OUTCOME,
    fail => task::FAIL, detail => task::DETAIL,
    at_file => task::AT_FILE, at_file_sz => task::AT_FILE_SZ, at_line => task::AT_LINE, phase_cnt => task::PHASE_CNT, phase_ids => task::PHASE_IDS, phase_vals => task::PHASE_VALS,
);

/// The name of the file `stream`'s want `id` opens, its path's last token,
/// as text: its quadkey, or its number in decimal. It is zero past its end,
/// and all zero for a want that opens no file.
fn path_bytes(stream: Stream, id: u64) -> [u8; TASK_PATH_SZ] {
    let mut out = Text::EMPTY;
    // A load's file is the loader's to name; any other want's, the files'.
    let path = match stream.row().source {
        Source::Load => load::of_id(id).map(|(_, load)| load.path()),
        _ => file::path_of(stream, id),
    }
    .unwrap_or_default();
    match path.last() {
        Some(Token::Int(n)) => write!(out, "{n}"),
        Some(Token::Str(key)) => out.write_str(key),
        None => Ok(()),
    }
    .or_bug(Errno::EOVERFLOW);
    out.bytes
}

impl TaskRecord {
    /// The row of `a`, which ended at `at` with `exit`, and the phase timings
    /// its run reported. `name_at` gives a failure's file name its offset in
    /// the profiler's string table, or none where the table has no room.
    pub(crate) fn of(
        a: &Attempt,
        at: f64,
        exit: Exit,
        phases: Option<PhaseList>,
        name_at: &mut dyn FnMut(&str) -> Option<u32>,
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
        match &exit {
            Exit::Failed(fail) => {
                row.fail = f64::from(fail.errnum as i32);
                row.detail = fail.detail as f64;
            }
            Exit::Refused(_) => row.fail = f64::from(Errno::ENOMEM as i32),
            Exit::Ok | Exit::Aborted => {}
        }
        if let Exit::Failed(fail) = exit {
            if let Some(off) = name_at(&fail.file) {
                row.at_file = f64::from(off);
                row.at_file_sz = fail.file.len() as f64;
            }
            row.at_line = f64::from(fail.line);
        }
        for (k, (name, val)) in phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name as u32 as f64;
            row.phase_vals[k] = val;
            row.phase_cnt = (k + 1) as f64;
        }
        row
    }
}
