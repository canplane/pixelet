// pxl-kernel/perf/section.rs

//! Every STAT and NOW section as a record held to perf.json by `words!`.

use core::mem::{offset_of, size_of};

use crate::perf::frame::{FrameSample, RaySample, EFFECTS, GPU_SPANS};
use crate::perf::spec::{FRAME, LIMIT, NOW, PERCENTILES, STAT, STEP_STAT, STREAM_STAT, WAKES};
use crate::perf::word::{words, zeroed, Words};
use crate::sys::io::demand_spec::{STREAM_NAMES, TASK_PHASE_NAMES};
use crate::sys::sched::phase_spec::PHASES;
use pxl_render::gpu::span_spec::FRAME_KINDS;

const REASONS: usize = LIMIT::REASONS as usize;
const REASON_WORDS: usize = LIMIT::REASON_WORDS as usize;
const STEP_SLOTS: usize = LIMIT::STEP_SLOTS as usize;
pub const TASK_PHASES: usize = TASK_PHASE_NAMES;
const CATEGORIES: usize = STREAM_NAMES;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percentiles {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

words!(Percentiles, PERCENTILES::WORDS, p50 => PERCENTILES::p50, p95 => PERCENTILES::p95, max => PERCENTILES::max);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingStateSection {
    pub recording: f64,
    pub elapsed_ms: f64,
    pub truncated: f64,
}

words!(RecordingStateSection, NOW::recording_state::WORDS,
    recording => NOW::recording_state::recording, elapsed_ms => NOW::recording_state::elapsed_ms, truncated => NOW::recording_state::truncated,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FramesSection {
    /// 0 for a window with no frames, where every other word is 0 too.
    pub present: f64,
    pub cnt: f64,
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
    pub interval: Percentiles,
    pub busy: Percentiles,
    pub tick: Percentiles,
    /// The other parts of `busy`, and what they leave of it, unmeasured.
    pub effects: Percentiles,
    pub stage: Percentiles,
    pub tasks: Percentiles,
    pub perf: Percentiles,
    pub rest: Percentiles,
    /// Each effect's p95, in EFFECT_NAMES' order.
    pub effects_p95: [f64; EFFECTS],
    /// The GPU's time: the time its spans ran, those that overlap counted
    /// once.
    pub gpu_busy: Percentiles,
    /// The busy time's p50 and p95 of the frames of each of the renderer's
    /// FRAME_KINDS a read timed, and how many they are, three a kind, all 0
    /// for a kind none was.
    pub gpu_busy_kinds: [f64; 3 * FRAME_KINDS],
    /// The p95 alone of each phase and each span: what is read of them; a
    /// span's over the frames a read timed that ran it, `gpu_spans_cnt` of them.
    pub phases_p95: [f64; PHASES],
    pub gpu_spans_p95: [f64; GPU_SPANS],
    pub gpu_spans_cnt: [f64; GPU_SPANS],
    /// Frames by the renderer's FRAME_KINDS, and turns' last ask by WAKES.
    pub kinds: [f64; FRAME_KINDS],
    pub wakes: [f64; WAKES],
    /// The world's pixels of the last frame a read timed.
    pub gpu_px: f64,
    /// The ray counts of the window's frames, summed lane by lane.
    pub rays: RaySample,
    /// The window's first frame and its last.
    pub first: FrameSample,
    pub last: FrameSample,
}

const _: () = assert!(
    STAT::frames::last + FRAME::WORDS <= STAT::frames::WORDS,
    "the last frame runs past STAT's frames section"
);

impl Default for FramesSection {
    fn default() -> Self {
        zeroed()
    }
}

words!(FramesSection, STAT::frames::WORDS,
    present => STAT::frames::present, cnt => STAT::frames::cnt, seconds => STAT::frames::seconds,
    moving => STAT::frames::moving, moving_ms => STAT::frames::moving_ms,
    missed => STAT::frames::missed, overran => STAT::frames::overran,
    long => STAT::frames::long, turns => STAT::frames::turns, gpu_read => STAT::frames::gpu_read,
    interval => STAT::frames::interval, busy => STAT::frames::busy,
    tick => STAT::frames::tick, effects => STAT::frames::effects,
    stage => STAT::frames::stage, tasks => STAT::frames::tasks,
    perf => STAT::frames::perf, rest => STAT::frames::rest,
    effects_p95 => STAT::frames::effects_p95, gpu_busy => STAT::frames::gpu_busy,
    gpu_busy_kinds => STAT::frames::gpu_busy_kinds,
    phases_p95 => STAT::frames::phases_p95, gpu_spans_p95 => STAT::frames::gpu_spans_p95,
    gpu_spans_cnt => STAT::frames::gpu_spans_cnt,
    kinds => STAT::frames::kinds, wakes => STAT::frames::wakes,
    gpu_px => STAT::frames::gpu_px, rays => STAT::frames::rays,
    first => STAT::frames::first, last => STAT::frames::last,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreamStats {
    pub cnt: f64,
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
    cnt => STREAM_STAT::cnt, ok => STREAM_STAT::ok, aborted => STREAM_STAT::aborted, err => STREAM_STAT::err,
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
    pub hit_kind: f64,
    pub pick_zoom: f64,
    pub ray_zoom: f64,
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
    cell => NOW::view::cell, hit_kind => NOW::view::hit_kind, pick_zoom => NOW::view::pick_zoom, ray_zoom => NOW::view::ray_zoom,
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

/// What the frames held at the most or the least since the recording
/// started (perf.json STAT.recording), each frame turn's folded in (`fold`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingSection {
    pub frames: f64,
    pub heap_live_max_bytes: f64,
    pub heap_run_max_bytes: f64,
    pub heap_slot_max_bytes: f64,
    pub heap_range_max_bytes: f64,
    pub heap_largest_free_min_bytes: f64,
    pub heap_refusals: f64,
    pub ring_cq_peak: f64,
    pub ring_sq_peak: f64,
    pub ring_left_peak: f64,
    pub body_slots_peak: f64,
    pub cap_terrain_refused: f64,
}

impl RecordingSection {
    /// One frame turn's figures, `h`, folded in: each the most, the
    /// largest free block the least, the refusals as they stand, the tiles
    /// refused summed.
    pub fn fold(&mut self, h: &RecordingSection) {
        let first = self.frames == 0.0;
        let max = |a: f64, b: f64| if first { b } else { a.max(b) };
        *self = RecordingSection {
            frames: self.frames + 1.0,
            heap_live_max_bytes: max(self.heap_live_max_bytes, h.heap_live_max_bytes),
            heap_run_max_bytes: max(self.heap_run_max_bytes, h.heap_run_max_bytes),
            heap_slot_max_bytes: max(self.heap_slot_max_bytes, h.heap_slot_max_bytes),
            heap_range_max_bytes: max(self.heap_range_max_bytes, h.heap_range_max_bytes),
            heap_largest_free_min_bytes: if first {
                h.heap_largest_free_min_bytes
            } else {
                self.heap_largest_free_min_bytes
                    .min(h.heap_largest_free_min_bytes)
            },
            heap_refusals: h.heap_refusals,
            ring_cq_peak: max(self.ring_cq_peak, h.ring_cq_peak),
            ring_sq_peak: max(self.ring_sq_peak, h.ring_sq_peak),
            ring_left_peak: max(self.ring_left_peak, h.ring_left_peak),
            body_slots_peak: max(self.body_slots_peak, h.body_slots_peak),
            cap_terrain_refused: self.cap_terrain_refused + h.cap_terrain_refused,
        };
    }
}

words!(RecordingSection, STAT::recording::WORDS,
    frames => STAT::recording::frames, heap_live_max_bytes => STAT::recording::heap_live_max_bytes,
    heap_run_max_bytes => STAT::recording::heap_run_max_bytes, heap_slot_max_bytes => STAT::recording::heap_slot_max_bytes,
    heap_range_max_bytes => STAT::recording::heap_range_max_bytes,
    heap_largest_free_min_bytes => STAT::recording::heap_largest_free_min_bytes,
    heap_refusals => STAT::recording::heap_refusals, ring_cq_peak => STAT::recording::ring_cq_peak,
    ring_sq_peak => STAT::recording::ring_sq_peak, ring_left_peak => STAT::recording::ring_left_peak,
    body_slots_peak => STAT::recording::body_slots_peak, cap_terrain_refused => STAT::recording::cap_terrain_refused,
);
