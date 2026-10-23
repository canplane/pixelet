// pxl-kernel/profiler/block.rs

//! Every summary section as a record held to profiler.json by `words!`.
//! Written word by word, sections had shifted lanes, mirrored structures and
//! unread fields that only an audit found (2026-09-23); a record makes each
//! of those a compile error.

use core::mem::{offset_of, size_of};

use crate::io::spec::{STREAM_NAMES, TASK_PHASE_NAMES};
use crate::profiler::frame::{FrameSample, GPU_SPANS};
use crate::profiler::spec::{FRAME, LIMITS, SPREAD, STEP, SUMMARY, TASK_SUMMARY, WAKES};
use crate::profiler::word::{words, Words};
use pxl_render::gpu::span_spec::FRAME_KINDS;
use xpute_guest::sched::tick::PHASES;

const REASONS: usize = LIMITS::REASONS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;
pub const TASK_PHASES: usize = TASK_PHASE_NAMES.len();
const CATEGORIES: usize = STREAM_NAMES.len();

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
    pub truncated: f64,
}

words!(RecBlock, SUMMARY::rec::WORDS,
    recording => SUMMARY::rec::recording, elapsed_ms => SUMMARY::rec::elapsed_ms, truncated => SUMMARY::rec::truncated,
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
    /// Intervals whose turns ran past them.
    pub long: f64,
    pub turns: f64,
    /// Frames a GPU read landed with: the GPU's figures are over these alone.
    pub gpu_read: f64,
    pub interval: Spread,
    pub busy: Spread,
    pub tick: Spread,
    pub gpu: Spread,
    /// The p95 alone of each phase and each span: what is read of them.
    pub phases_p95: [f64; PHASES.len()],
    pub gpu_spans_p95: [f64; GPU_SPANS],
    /// Frames by the renderer's FRAME_KINDS, and turns' last ask by WAKES.
    pub kinds: [f64; FRAME_KINDS.len()],
    pub wakes: [f64; WAKES.len()],
    pub heap_used_delta_bytes: f64,
    pub last: FrameSample,
}

const _: () = assert!(SUMMARY::frames::last + FRAME::WORDS <= SUMMARY::frames::WORDS, "the last frame runs past the summary's frames block");

words!(FramesBlock, SUMMARY::frames::WORDS,
    present => SUMMARY::frames::present, n => SUMMARY::frames::n, seconds => SUMMARY::frames::seconds,
    moving => SUMMARY::frames::moving, missed => SUMMARY::frames::missed, overran => SUMMARY::frames::overran,
    long => SUMMARY::frames::long, turns => SUMMARY::frames::turns, gpu_read => SUMMARY::frames::gpu_read,
    interval => SUMMARY::frames::interval, busy => SUMMARY::frames::busy,
    tick => SUMMARY::frames::tick, gpu => SUMMARY::frames::gpu,
    phases_p95 => SUMMARY::frames::phases_p95, gpu_spans_p95 => SUMMARY::frames::gpu_spans_p95,
    kinds => SUMMARY::frames::kinds, wakes => SUMMARY::frames::wakes,
    heap_used_delta_bytes => SUMMARY::frames::heap_used_delta_bytes,
    last => SUMMARY::frames::last,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskBlock {
    pub n: f64,
    pub ok: f64,
    pub aborted: f64,
    pub error: f64,
    pub lat_p50: f64,
    pub lat_p95: f64,
    pub reason_count: f64,
    /// (reason, status, count) each, commonest first.
    pub reasons: [[f64; 3]; REASONS],
    pub phase_n: f64,
    /// NaN for a phase no run in the window reported.
    pub phase_mean: [f64; TASK_PHASES],
}

words!(TaskBlock, TASK_SUMMARY::WORDS,
    n => TASK_SUMMARY::n, ok => TASK_SUMMARY::ok, aborted => TASK_SUMMARY::aborted, error => TASK_SUMMARY::error,
    lat_p50 => TASK_SUMMARY::lat_p50, lat_p95 => TASK_SUMMARY::lat_p95, reason_count => TASK_SUMMARY::reason_count, reasons => TASK_SUMMARY::reasons,
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
pub struct ViewBlock {
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
    pub tau: f64,
    pub center_lon: f64,
    pub center_lat: f64,
    pub center_z: f64,
    pub hover_x: f64,
    pub hover_y: f64,
    pub hover_z: f64,
    pub hover_lon: f64,
    pub hover_lat: f64,
    pub hover_alt: f64,
    pub cell_want: f64,
}

words!(ViewBlock, SUMMARY::view::WORDS,
    dist => SUMMARY::view::dist, polar => SUMMARY::view::polar, azimuth => SUMMARY::view::azimuth, tau => SUMMARY::view::tau,
    center_lon => SUMMARY::view::center_lon, center_lat => SUMMARY::view::center_lat, center_z => SUMMARY::view::center_z,
    hover_x => SUMMARY::view::hover_x, hover_y => SUMMARY::view::hover_y, hover_z => SUMMARY::view::hover_z,
    hover_lon => SUMMARY::view::hover_lon, hover_lat => SUMMARY::view::hover_lat, hover_alt => SUMMARY::view::hover_alt,
    cell_want => SUMMARY::view::cell_want,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepBlock {
    /// Indices into PHASES and STEP_NAMES.
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
