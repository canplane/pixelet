// pxl-kernel/sched/ledger/block.rs

//! Every section of the summary as one record, laid out as ledger.json lays
//! the section out and held to it by `words!` (record.rs).
//!
//! The summary used to be written word by word — `out[at(a, F::field)] = v`
//! for a scalar and a hand-ordered slice for a whole section — and the block
//! was then the only place the wiring existed, in a form nothing could check.
//! Three faults followed from that one shape, and an audit rather than the
//! compiler is what found them (2026-09-23): a section whose slice put two
//! values in the middle of lanes the JSON had appended to the end, so fifteen
//! of its twenty lanes carried a neighbor's number; a lane fed from a
//! structure that mirrored the one it was about; and fields computed for the
//! ledger that no lane read.
//!
//! A record closes all three. A lane with no field is a size mismatch, a
//! field at the wrong word is an offset mismatch, and a field nobody fills is
//! a struct literal that does not compile — so none of them is a thing to go
//! and check. What a field is filled *from* is still a judgment, but it is
//! one line beside the field's own name, in one place.

use core::mem::{offset_of, size_of};

use crate::sched::ledger::record::{words, FrameSample, Words, CENSUS_LEN, GPU_SPANS, LEVELS};
use crate::sched::ledger::spec::{FRAME, LIMITS, PHASE_NAMES, SPREAD, STEP, SUMMARY, TASK_PHASE_NAMES, TASK_SUMMARY};

const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;
const TASK_PHASES: usize = TASK_PHASE_NAMES.len();
const CATEGORIES: usize = crate::sched::ledger::spec::CATEGORY_NAMES.len();

/// What a set of values was, at the two points worth naming and at its worst.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

words!(Spread, SPREAD::WORDS, p50 => SPREAD::p50, p95 => SPREAD::p95, max => SPREAD::max);

/// Whether the ledger is recording, and how much it holds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecBlock {
    pub recording: f64,
    pub elapsed_ms: f64,
    pub tasks: f64,
    pub frames: f64,
    pub truncated: f64,
}

words!(RecBlock, SUMMARY::rec::WORDS,
    recording => SUMMARY::rec::recording, elapsed_ms => SUMMARY::rec::elapsed_ms,
    tasks => SUMMARY::rec::tasks, frames => SUMMARY::rec::frames, truncated => SUMMARY::rec::truncated,
);

/// The window's frames: how many and over how long, the timings as spreads,
/// the work rates, and the last frame whole.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramesBlock {
    /// 0 for a window with no frames in it, where every other word is 0 too.
    pub present: f64,
    pub n: f64,
    pub seconds: f64,
    pub moving: f64,
    pub missed: f64,
    pub overran: f64,
    pub dt: Spread,
    pub task: Spread,
    pub offthread: Spread,
    pub gpu: Spread,
    pub sched: Spread,
    pub chunks: Spread,
    pub phases: [Spread; PHASE_NAMES.len()],
    pub gpu_spans: [Spread; GPU_SPANS],
    pub work_scans: f64,
    pub work_bakes: f64,
    pub per_s_scans: f64,
    pub per_s_bakes: f64,
    pub backlog_s: f64,
    pub backlog_per_s_scans: f64,
    pub backlog_per_s_bakes: f64,
    pub chunks_per_bake: f64,
    pub mounts_added: f64,
    pub mounts_removed: f64,
    pub used_delta_mb: f64,
    pub census: [[f64; 2]; CENSUS_LEN],
    pub level_tris: [f64; LEVELS],
    pub last: FrameSample,
}

const _: () = assert!(SUMMARY::frames::last + FRAME::WORDS <= SUMMARY::frames::WORDS, "the last frame runs past the summary's frames block");

words!(FramesBlock, SUMMARY::frames::WORDS,
    present => SUMMARY::frames::present, n => SUMMARY::frames::n, seconds => SUMMARY::frames::seconds,
    moving => SUMMARY::frames::moving, missed => SUMMARY::frames::missed, overran => SUMMARY::frames::overran,
    dt => SUMMARY::frames::dt, task => SUMMARY::frames::task, offthread => SUMMARY::frames::offthread,
    gpu => SUMMARY::frames::gpu, sched => SUMMARY::frames::sched, chunks => SUMMARY::frames::chunks,
    phases => SUMMARY::frames::phases, gpu_spans => SUMMARY::frames::gpu_spans,
    work_scans => SUMMARY::frames::work_scans, work_bakes => SUMMARY::frames::work_bakes,
    per_s_scans => SUMMARY::frames::per_s_scans, per_s_bakes => SUMMARY::frames::per_s_bakes,
    backlog_s => SUMMARY::frames::backlog_s, backlog_per_s_scans => SUMMARY::frames::backlog_per_s_scans,
    backlog_per_s_bakes => SUMMARY::frames::backlog_per_s_bakes, chunks_per_bake => SUMMARY::frames::chunks_per_bake,
    mounts_added => SUMMARY::frames::mounts_added, mounts_removed => SUMMARY::frames::mounts_removed,
    used_delta_mb => SUMMARY::frames::used_delta_mb, census => SUMMARY::frames::census,
    level_tris => SUMMARY::frames::level_tris, last => SUMMARY::frames::last,
);

/// One category of task over the window.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskBlock {
    pub n: f64,
    pub ok: f64,
    pub aborted: f64,
    pub error: f64,
    pub lat: Spread,
    /// How many of the reason slots are filled, commonest first.
    pub reason_count: f64,
    /// (reason, status, count) each.
    pub reasons: [[f64; 3]; REASONS],
    pub phase_n: f64,
    /// NaN for a phase no run in the window reported.
    pub phase_mean: [f64; TASK_PHASES],
}

words!(TaskBlock, TASK_SUMMARY::WORDS,
    n => TASK_SUMMARY::n, ok => TASK_SUMMARY::ok, aborted => TASK_SUMMARY::aborted, error => TASK_SUMMARY::error,
    lat => TASK_SUMMARY::lat, reason_count => TASK_SUMMARY::reason_count, reasons => TASK_SUMMARY::reasons,
    phase_n => TASK_SUMMARY::phase_n, phase_mean => TASK_SUMMARY::phase_mean,
);

/// Every category's, as the block holds them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TasksBlock {
    pub slots: [TaskBlock; CATEGORIES],
}

words!(TasksBlock, SUMMARY::tasks::WORDS, slots => SUMMARY::tasks::slots);

/// The counters the log keeps over the window (ledger/log.rs).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CountersBlock {
    pub faces_kept: f64,
    pub faces_possible: f64,
    pub runs_culled: f64,
    pub runs_total: f64,
    /// Boxes a coarse tier's mesher made — a count, and not the peak a slot
    /// is sized against (`FramesBlock::last.work.coarse_peak`).
    pub coarse_boxes: f64,
    pub coarse_low_fill: f64,
    pub bands_reused: f64,
    pub bands_allocated: f64,
    pub built_direct: f64,
    pub built_scans: f64,
    pub built_bakes: f64,
    pub built_starved: f64,
    pub decor_dropped: f64,
    pub atlas_repacks: f64,
    pub atlas_uploads: f64,
}

impl CountersBlock {
    /// Each counter less `b`'s: the window's share of what accrued since
    /// load. Word by word, as the block is laid out, so a counter added to
    /// the block is subtracted without being named here.
    pub fn minus(mut self, b: &CountersBlock) -> CountersBlock {
        let n = size_of::<CountersBlock>() / 8;
        // SAFETY: `words!` below holds the block to plain f64s end to end.
        let (a, b) = unsafe {
            (
                core::slice::from_raw_parts_mut(&mut self as *mut _ as *mut f64, n),
                core::slice::from_raw_parts(b as *const _ as *const f64, n),
            )
        };
        for (x, y) in a.iter_mut().zip(b) {
            *x -= y;
        }
        self
    }
}

words!(CountersBlock, SUMMARY::counters::WORDS,
    faces_kept => SUMMARY::counters::faces_kept, faces_possible => SUMMARY::counters::faces_possible,
    runs_culled => SUMMARY::counters::runs_culled, runs_total => SUMMARY::counters::runs_total,
    coarse_boxes => SUMMARY::counters::coarse_boxes, coarse_low_fill => SUMMARY::counters::coarse_low_fill,
    bands_reused => SUMMARY::counters::bands_reused, bands_allocated => SUMMARY::counters::bands_allocated,
    built_direct => SUMMARY::counters::built_direct, built_scans => SUMMARY::counters::built_scans,
    built_bakes => SUMMARY::counters::built_bakes, built_starved => SUMMARY::counters::built_starved,
    decor_dropped => SUMMARY::counters::decor_dropped, atlas_repacks => SUMMARY::counters::atlas_repacks,
    atlas_uploads => SUMMARY::counters::atlas_uploads,
);

/// The badge sheet.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AtlasBlock {
    pub size: f64,
    pub entries: f64,
    pub shelves: f64,
    pub used_px: f64,
}

words!(AtlasBlock, SUMMARY::atlas::WORDS,
    size => SUMMARY::atlas::size, entries => SUMMARY::atlas::entries,
    shelves => SUMMARY::atlas::shelves, used_px => SUMMARY::atlas::used_px,
);

/// What the chunks keep, against the budget they keep it under.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RetainBlock {
    pub run_sets: f64,
    pub run_set_mb: f64,
    pub tiers: f64,
    pub decor_steps: f64,
    pub decor_instances: f64,
    pub on_bytes: f64,
    pub off_bytes: f64,
    pub budget_bytes: f64,
}

words!(RetainBlock, SUMMARY::retain::WORDS,
    run_sets => SUMMARY::retain::run_sets, run_set_mb => SUMMARY::retain::run_set_mb,
    tiers => SUMMARY::retain::tiers, decor_steps => SUMMARY::retain::decor_steps,
    decor_instances => SUMMARY::retain::decor_instances, on_bytes => SUMMARY::retain::on_bytes,
    off_bytes => SUMMARY::retain::off_bytes, budget_bytes => SUMMARY::retain::budget_bytes,
);

/// The fetch scheduler's queue, as it stands.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FetchBlock {
    pub queued: f64,
    pub running: f64,
    pub credits: f64,
}

words!(FetchBlock, SUMMARY::fetch::WORDS,
    queued => SUMMARY::fetch::queued, running => SUMMARY::fetch::running, credits => SUMMARY::fetch::credits,
);

/// One step of the tick, as the window saw it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepBlock {
    /// An index into PHASE_NAMES, and one into STEP_NAMES.
    pub phase: f64,
    pub name: f64,
    pub ms: f64,
    pub count: f64,
    pub max_ms: f64,
}

words!(StepBlock, STEP::WORDS,
    phase => STEP::phase, name => STEP::name, ms => STEP::ms, count => STEP::count, max_ms => STEP::max_ms,
);

/// The steps that ran, and how many of the slots they filled.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepsBlock {
    pub count: f64,
    pub slots: [StepBlock; STEP_SLOTS],
}

words!(StepsBlock, SUMMARY::steps::WORDS, count => SUMMARY::steps::count, slots => SUMMARY::steps::slots);
