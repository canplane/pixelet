// pxl-kernel/profiler/task.rs

//! A task's row in the profiler's lanes: what ran, when, and what it came
//! to. The work that ran writes it (io/request.rs `record`).

use core::mem::{offset_of, size_of};

use crate::io::demand::PHASES_MAX;
use crate::profiler::spec::TASK;
use crate::profiler::word::{words, Words};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskRow {
    pub category: f64,
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
    pub phase_count: f64,
    pub phase_ids: [f64; PHASES_MAX],
    pub phase_values: [f64; PHASES_MAX],
}

words!(TaskRow, TASK::WORDS,
    category => TASK::category, subject => TASK::subject, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_count => TASK::phase_count, phase_ids => TASK::phase_ids, phase_values => TASK::phase_values,
);
