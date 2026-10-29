// pxl-kernel/profiler/task.rs

//! A task's row in the profiler's lanes: what ran, when, and what it came
//! to. The work that ran writes it (sys/io/request.rs `record`).

use core::mem::{offset_of, size_of};

use crate::profiler::spec::TASK;
use crate::profiler::word::{words, Words};
use crate::sys::io::demand::PHASES_MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRow {
    pub stream: f64,
    pub subject: u64,
    pub priority: f64,
    pub enqueue_ms: f64,
    /// NaN for a task that never ran.
    pub start_ms: f64,
    pub end_ms: f64,
    pub outcome: f64,
    pub fail: f64,
    pub detail: f64,
    /// Failure location: file name address and length, and line. 0 when the host raised it.
    pub at_file: f64,
    pub at_file_bytes: f64,
    pub at_line: f64,
    pub phase_cnt: f64,
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_vals: [f64; PHASES_MAX],
}

words!(TaskRow, TASK::WORDS,
    stream => TASK::stream, subject => TASK::subject, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_cnt => TASK::phase_cnt, phase_ids => TASK::phase_ids, phase_vals => TASK::phase_vals,
);
