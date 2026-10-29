// pxl-kernel/profiler/section.rs

//! Every STAT and NOW section as a record held to profiler.json by `words!`.
//! Written word by word, sections had shifted lanes, mirrored structures and
//! unread fields that only an audit found (2026-09-23); a record makes each
//! of those a compile error.

use core::mem::{offset_of, size_of};

use crate::profiler::frame::{FrameSample, EFFECTS, GPU_SPANS};
use crate::profiler::spec::{FRAME, LIMITS, SPREAD, STEP, SUMMARY, TASK_SUMMARY, WAKES};
use crate::profiler::word::{words, Words};
use crate::sys::io::demand_spec::{STREAM_NAMES, TASK_PHASE_NAMES};
use pxl_render::gpu::span_spec::FRAME_KINDS;
use xpute_guest::sched::tick::PHASES;

const REASONS: usize = LIMITS::REASONS as usize;
const REASON_WORDS: usize = LIMITS::REASON_WORDS as usize;
const STEP_SLOTS: usize = LIMITS::STEP_SLOTS as usize;
pub const TASK_PHASES: usize = TASK_PHASE_NAMES;
const CATEGORIES: usize = STREAM_NAMES;

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
pub struct RecSection {
    pub recording: f64,
    pub elapsed_ms: f64,
    pub truncated: f64,
}

words!(RecSection, NOW::rec::WORDS,
    recording => NOW::rec::recording, elapsed_ms => NOW::rec::elapsed_ms, truncated => NOW::rec::truncated,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramesSection {
    /// 0 for a window with no frames, where every other word is 0 too.
    pub present: f64,
    pub n: f64,
    pub seconds: f64,
    pub moving: f64,
    /// The moving frames' intervals summed: a second over their mean
    /// interval is the rate they were drawn at.
    pub moving_ms: f64,
    pub missed: f64,
    pub overran: f64,
    /// Intervals whose turns ran past them.
    pub long: f64,
    pub turns: f64,
    /// Frames a GPU read timed: the GPU's figures are over these alone.
    pub gpu_read: f64,
    pub interval: Spread,
    pub busy: Spread,
    pub tick: Spread,
    /// The other parts of `busy`, and what they leave of it, unmeasured.
    pub effects: Spread,
    pub stage: Spread,
    pub tasks: Spread,
    pub profiler: Spread,
    pub rest: Spread,
    /// Each effect's p95, in EFFECT_NAMES' order.
    pub effects_p95: [f64; EFFECTS],
    pub gpu: Spread,
    /// The p95 alone of each phase and each span: what is read of them.
    pub phases_p95: [f64; PHASES.len()],
    pub gpu_spans_p95: [f64; GPU_SPANS],
    /// Frames by the renderer's FRAME_KINDS, and turns' last ask by WAKES.
    pub kinds: [f64; FRAME_KINDS],
    pub wakes: [f64; WAKES],
    pub heap_used_delta_bytes: f64,
    /// The window's first frame and its last: what is summed since boot is
    /// the window's as the one less the other.
    pub first: FrameSample,
    pub last: FrameSample,
}

const _: () = assert!(
    STAT::frames::last + FRAME::WORDS <= STAT::frames::WORDS,
    "the last frame runs past STAT's frames section"
);

words!(FramesSection, SUMMARY::frames::WORDS,
    present => SUMMARY::frames::present, n => SUMMARY::frames::n, seconds => SUMMARY::frames::seconds,
    moving => SUMMARY::frames::moving, moving_ms => SUMMARY::frames::moving_ms,
    missed => SUMMARY::frames::missed, overran => SUMMARY::frames::overran,
    long => SUMMARY::frames::long, turns => SUMMARY::frames::turns, gpu_read => SUMMARY::frames::gpu_read,
    interval => SUMMARY::frames::interval, busy => SUMMARY::frames::busy,
    tick => SUMMARY::frames::tick, effects => SUMMARY::frames::effects,
    stage => SUMMARY::frames::stage, tasks => SUMMARY::frames::tasks,
    profiler => SUMMARY::frames::profiler, rest => SUMMARY::frames::rest,
    effects_p95 => SUMMARY::frames::effects_p95, gpu => SUMMARY::frames::gpu,
    phases_p95 => SUMMARY::frames::phases_p95, gpu_spans_p95 => SUMMARY::frames::gpu_spans_p95,
    kinds => SUMMARY::frames::kinds, wakes => SUMMARY::frames::wakes,
    heap_used_delta_bytes => SUMMARY::frames::heap_used_delta_bytes,
    first => SUMMARY::frames::first, last => SUMMARY::frames::last,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreamStats {
    pub n: f64,
    pub ok: f64,
    pub aborted: f64,
    pub err: f64,
    pub lat_p50: f64,
    pub lat_p95: f64,
    pub reason_cnt: f64,
    /// (reason, status, count) each, commonest first.
    pub reasons: [[f64; REASON_WORDS]; REASONS],
    pub phase_n: f64,
    /// NaN for a phase no run in the window reported.
    pub phase_mean: [f64; TASK_PHASES],
}

words!(StreamStats, STREAM_STAT::WORDS,
    n => STREAM_STAT::n, ok => STREAM_STAT::ok, aborted => STREAM_STAT::aborted, err => STREAM_STAT::err,
    lat_p50 => STREAM_STAT::lat_p50, lat_p95 => STREAM_STAT::lat_p95, reason_cnt => STREAM_STAT::reason_cnt, reasons => STREAM_STAT::reasons,
    phase_n => STREAM_STAT::phase_n, phase_mean => STREAM_STAT::phase_mean,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TasksSection {
    pub slots: [StreamStats; CATEGORIES],
}

words!(TasksSection, STAT::tasks::WORDS, slots => STAT::tasks::slots);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSection {
    pub eye_x: f64,
    pub eye_y: f64,
    pub eye_z: f64,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
    pub pivot_x: f64,
    pub pivot_y: f64,
    pub pivot_z: f64,
    pub pivot_lon: f64,
    pub pivot_lat: f64,
    pub pivot_alt: f64,
    pub hover_x: f64,
    pub hover_y: f64,
    pub hover_z: f64,
    pub cell: f64,
    pub struck: f64,
    pub held: f64,
    pub drawn: f64,
    pub folded: f64,
    pub place_x: f64,
    pub place_y: f64,
    pub place_z: f64,
    pub place_zoom: f64,
    pub blocked: f64,
}

words!(ViewSection, NOW::view::WORDS,
    eye_x => NOW::view::eye_x, eye_y => NOW::view::eye_y, eye_z => NOW::view::eye_z,
    dist => NOW::view::dist, polar => NOW::view::polar, azimuth => NOW::view::azimuth,
    pivot_x => NOW::view::pivot_x, pivot_y => NOW::view::pivot_y, pivot_z => NOW::view::pivot_z,
    pivot_lon => NOW::view::pivot_lon, pivot_lat => NOW::view::pivot_lat, pivot_alt => NOW::view::pivot_alt,
    hover_x => NOW::view::hover_x, hover_y => NOW::view::hover_y, hover_z => NOW::view::hover_z,
    cell => NOW::view::cell, struck => NOW::view::struck, held => NOW::view::held, drawn => NOW::view::drawn,
    folded => NOW::view::folded, place_x => NOW::view::place_x, place_y => NOW::view::place_y, place_z => NOW::view::place_z,
    place_zoom => NOW::view::place_zoom, blocked => NOW::view::blocked,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepStats {
    /// Indices into PHASES and STEP_NAMES.
    pub phase: f64,
    pub name: f64,
    pub ms: f64,
    pub cnt: f64,
    pub max_ms: f64,
}

words!(StepStats, STEP_STAT::WORDS,
    phase => STEP_STAT::phase, name => STEP_STAT::name, ms => STEP_STAT::ms, cnt => STEP_STAT::cnt, max_ms => STEP_STAT::max_ms,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepsSection {
    pub cnt: f64,
    pub slots: [StepStats; STEP_SLOTS],
}

words!(StepsSection, STAT::steps::WORDS, cnt => STAT::steps::cnt, slots => STAT::steps::slots);
