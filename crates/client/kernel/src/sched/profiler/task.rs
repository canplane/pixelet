// pxl-kernel/sched/profiler/task.rs

//! A task's record: what ran, when, and what it came to.

use core::mem::{offset_of, size_of};
use std::collections::HashMap;

use xpute_guest::global::Global;
use xpute_guest::mem::base::off_of_slice;

use crate::io::demand::{DemandKey, Phases};
use crate::io::failure::{Failure, Outcome};
use crate::sched::profiler::spec::{LIMITS, TASK};
use crate::sched::profiler::word::{words, Words};

#[derive(Clone, Debug, PartialEq)]
pub struct TaskRecord {
    pub key: DemandKey,
    pub priority: f64,
    pub enqueue_ms: f64,
    pub start_ms: Option<f64>,
    pub end_ms: f64,
    /// None when the run reported no split.
    pub phases: Option<Phases>,
    pub outcome: Outcome,
    pub fail: Failure,
}

const PHASES_MAX: usize = LIMITS::PHASES_MAX as usize;
const _: () = assert!(PHASES_MAX == crate::io::demand::PHASES_MAX, "a task row holds every phase a run can report");

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

impl TaskRow {
    pub fn of(rec: &TaskRecord) -> TaskRow {
        let mut row = TaskRow {
            category: rec.key.category as u32 as f64,
            subject: rec.key.subject,
            priority: rec.priority,
            enqueue_ms: rec.enqueue_ms,
            start_ms: rec.start_ms.unwrap_or(f64::NAN),
            end_ms: rec.end_ms,
            outcome: rec.outcome as u32 as f64,
            fail: rec.fail.errno as f64,
            detail: rec.fail.detail as f64,
            ..TaskRow::default()
        };
        if let Some(at) = rec.fail.at {
            row.at_file = off_of_slice(file_in_memory(at.file()).as_bytes()) as f64;
            row.at_file_bytes = at.file().len() as f64;
            row.at_line = at.line() as f64;
        }
        for (k, (name, value)) in rec.phases.iter().flat_map(|p| p.iter()).enumerate() {
            row.phase_ids[k] = name as u32 as f64;
            row.phase_values[k] = value;
            row.phase_count = (k + 1) as f64;
        }
        row
    }
}

static FILES: Global<HashMap<usize, Box<str>>> = Global::new();

/// A location's file name lies outside kernel memory on a device, so rows
/// name a heap copy; the names are few and kept for good.
fn file_in_memory(file: &'static str) -> &'static str {
    FILES.get(HashMap::new).entry(file.as_ptr().addr()).or_insert_with(|| file.into())
}

words!(TaskRow, TASK::WORDS,
    category => TASK::category, subject => TASK::subject, priority => TASK::priority, enqueue_ms => TASK::enqueue_ms,
    start_ms => TASK::start_ms, end_ms => TASK::end_ms, outcome => TASK::outcome,
    fail => TASK::fail, detail => TASK::detail,
    at_file => TASK::at_file, at_file_bytes => TASK::at_file_bytes, at_line => TASK::at_line, phase_count => TASK::phase_count, phase_ids => TASK::phase_ids, phase_values => TASK::phase_values,
);
