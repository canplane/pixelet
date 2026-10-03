// pxl-kernel/sched/profiler/block.rs

//! Every summary section as a record held to profiler.json by `words!`.
//! Written word by word, sections had shifted lanes, mirrored structures and
//! unread fields that only an audit found (2026-09-23); a record makes each
//! of those a compile error.

use core::mem::{offset_of, size_of};

use crate::sched::profiler::record::{words, FrameSample, Words, CENSUS_LEN, GPU_SPANS, LEVELS};
use crate::sched::profiler::spec::{FRAME, LIMITS, PHASE_NAMES, SPREAD, STEP, SUMMARY, TASK_PHASE_NAMES, TASK_SUMMARY};

const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;
pub const TASK_PHASES: usize = TASK_PHASE_NAMES.len();
const CATEGORIES: usize = crate::sched::profiler::spec::CATEGORY_NAMES.len();

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

words!(Spread, SPREAD::WORDS, p50 => SPREAD::p50, p95 => SPREAD::p95, max => SPREAD::max);

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramesBlock {
    /// 0 for a window with no frames, where every other word is 0 too.
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskBlock {
    pub n: f64,
    pub ok: f64,
    pub aborted: f64,
    pub error: f64,
    pub lat: Spread,
    pub reason_count: f64,
    /// (reason, status, count) each, commonest first.
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TasksBlock {
    pub slots: [TaskBlock; CATEGORIES],
}

words!(TasksBlock, SUMMARY::tasks::WORDS, slots => SUMMARY::tasks::slots);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CountersBlock {
    pub faces_kept: f64,
    pub faces_possible: f64,
    pub runs_culled: f64,
    pub runs_total: f64,
    /// A count, not the peak a slot is sized against.
    pub coarse_boxes: f64,
    pub coarse_low_fill: f64,
    pub bands_reused: f64,
    pub bands_allocated: f64,
    pub built_direct: f64,
    pub built_scans: f64,
    pub built_bakes: f64,
    pub built_starved: f64,
    pub atlas_repacks: f64,
    pub atlas_uploads: f64,
}

impl CountersBlock {
    /// Word by word, so a counter added to the block is subtracted without being named here.
    pub fn minus(mut self, b: &CountersBlock) -> CountersBlock {
        let n = size_of::<CountersBlock>() / 8;
        // SAFETY: `words!` below holds the block to plain f64s end to end.
        let (a, b) = unsafe {
            (
                core::slice::from_raw_parts_mut((&raw mut self).cast::<f64>(), n),
                core::slice::from_raw_parts(std::ptr::from_ref(b).cast::<f64>(), n),
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
    atlas_repacks => SUMMARY::counters::atlas_repacks,
    atlas_uploads => SUMMARY::counters::atlas_uploads,
);

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeldBlock {
    pub run_sets: f64,
    pub run_set_mb: f64,
    pub tiers: f64,
    pub decor_blocks: f64,
    pub decor_instances: f64,
    pub decor_bytes: f64,
}

words!(HeldBlock, SUMMARY::held::WORDS,
    run_sets => SUMMARY::held::run_sets, run_set_mb => SUMMARY::held::run_set_mb,
    tiers => SUMMARY::held::tiers, decor_blocks => SUMMARY::held::decor_blocks,
    decor_instances => SUMMARY::held::decor_instances, decor_bytes => SUMMARY::held::decor_bytes,
);

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct QualityBlock {
    pub step: f64,
    pub star: f64,
    pub frames_held: f64,
    pub admitted: f64,
    pub evicted: f64,
    pub refined: f64,
    pub recubed: f64,
    pub changes_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapBlock {
    pub used_mb: f64,
    pub requests: f64,
}

words!(HeapBlock, SUMMARY::heap::WORDS, used_mb => SUMMARY::heap::used_mb, requests => SUMMARY::heap::requests);

words!(QualityBlock, SUMMARY::quality::WORDS,
    step => SUMMARY::quality::step, star => SUMMARY::quality::star, frames_held => SUMMARY::quality::frames_held,
    admitted => SUMMARY::quality::admitted, evicted => SUMMARY::quality::evicted,
    refined => SUMMARY::quality::refined, recubed => SUMMARY::quality::recubed, changes_peak => SUMMARY::quality::changes_peak,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepBlock {
    /// Indices into PHASE_NAMES and STEP_NAMES.
    pub phase: f64,
    pub name: f64,
    pub ms: f64,
    pub count: f64,
    pub max_ms: f64,
}

words!(StepBlock, STEP::WORDS,
    phase => STEP::phase, name => STEP::name, ms => STEP::ms, count => STEP::count, max_ms => STEP::max_ms,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepsBlock {
    pub count: f64,
    pub slots: [StepBlock; STEP_SLOTS],
}

words!(StepsBlock, SUMMARY::steps::WORDS, count => SUMMARY::steps::count, slots => SUMMARY::steps::slots);
