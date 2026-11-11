// pxl-core/perf/section.rs

//! Every STAT and NOW section of perf.json as a record, each held to its
//! layout by `words!`.

use core::mem::{offset_of, size_of};

use crate::perf::frame::{FrameSample, RaySample, EFFECTS, GPU_SPANS};
use crate::perf::spec::{FRAME, LIMIT, NOW, PERCENTILE, STAT, STEP_STAT, STREAM_STAT, WAKES};
use crate::perf::word::{words, zeroed, AsWords};
use crate::sys::io::demand_spec::{STREAM_NAMES, TASK_PHASE_NAMES};
use crate::sys::sched::phase_spec::PHASES;
use pxl_render::gpu::span_spec::FRAME_KINDS;

const REASONS: usize = LIMIT::REASONS as usize;
const REASON_WORDS: usize = LIMIT::REASON_WORDS as usize;
const STEP_CAP: usize = LIMIT::STEP_CAP as usize;
pub const TASK_PHASES: usize = TASK_PHASE_NAMES;
const CATEGORIES: usize = STREAM_NAMES;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percentile {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

words!(Percentile, PERCENTILE::WORDS, p50 => PERCENTILE::p50, p95 => PERCENTILE::p95, max => PERCENTILE::max);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RecordingStateSection {
    pub recording: f64,
    pub elapsed_ms: f64,
    pub truncated: f64,
    pub frames: f64,
    pub tasks: f64,
    pub worst: f64,
}

words!(RecordingStateSection, NOW::recording_state::WORDS,
    recording => NOW::recording_state::recording, elapsed_ms => NOW::recording_state::elapsed_ms, truncated => NOW::recording_state::truncated,
    frames => NOW::recording_state::frames, tasks => NOW::recording_state::tasks, worst => NOW::recording_state::worst,
);

/// Whether the core draws, as perf.json's NOW `draw` says.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawSection {
    pub turns: f64,
    pub frames: f64,
    pub refused_admit: f64,
    pub refused_stream: f64,
    pub generation: f64,
    pub status: f64,
    pub errors: f64,
    pub unfinished: f64,
}

words!(DrawSection, NOW::draw::WORDS,
    turns => NOW::draw::turns, frames => NOW::draw::frames, refused_admit => NOW::draw::refused_admit, refused_stream => NOW::draw::refused_stream,
    generation => NOW::draw::generation, status => NOW::draw::status, errors => NOW::draw::errors, unfinished => NOW::draw::unfinished,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameSection {
    /// 0 for a window with no frames, where every other word is 0 too.
    pub present: f64,
    pub cnt: f64,
    pub seconds: f64,
    pub moving: f64,
    /// The sum of the moving frames' intervals. Divided by `moving`, it gives
    /// their mean interval, and a second over that mean is the rate they were
    /// drawn at.
    pub moving_ms: f64,
    pub missed: f64,
    pub overran: f64,
    /// How many intervals had turns that ran past the interval's end.
    pub long: f64,
    pub turns: f64,
    /// How many frames a GPU read timed. The GPU's figures cover these frames
    /// alone.
    pub gpu_timed: f64,
    pub interval: Percentile,
    pub busy: Percentile,
    pub tick: Percentile,
    /// The other measured parts of `busy`, then `rest`, what they leave of it
    /// unmeasured.
    pub effect: Percentile,
    pub stage: Percentile,
    pub task: Percentile,
    pub perf: Percentile,
    pub rest: Percentile,
    /// Each effect's p95, indexed by `EffectName`.
    pub effects_p95: [f64; EFFECTS],
    /// The GPU's time, with overlapping spans counted once.
    pub gpu_busy: Percentile,
    /// Three words for each of the renderer's FRAME_KINDS: the p50 and p95 of
    /// the GPU's busy time over that kind's timed frames, and how many there
    /// were. All three are 0 for a kind no timed frame had.
    pub gpu_busy_kinds: [f64; 3 * FRAME_KINDS],
    /// Only the p95 of each phase and each GPU span is kept, because only it
    /// is read. A span's p95 covers the timed frames that ran the span, and
    /// `gpu_spans_cnt` counts those frames.
    pub phases_p95: [f64; PHASES],
    pub gpu_spans_p95: [f64; GPU_SPANS],
    pub gpu_spans_cnt: [f64; GPU_SPANS],
    /// How many frames fell in each of the renderer's FRAME_KINDS, and how
    /// many intervals ended on each of the WAKES as the last turn's ask.
    pub kinds: [f64; FRAME_KINDS],
    pub wakes: [f64; WAKES],
    /// The world's pixel count in the last frame a read timed.
    pub gpu_px: f64,
    /// The ray counts of the window's frames, summed field by field.
    pub ray_sample: RaySample,
    /// The window's first frame and its last.
    pub first: FrameSample,
    pub last: FrameSample,
}

const _: () = assert!(
    STAT::frames::last + FRAME::WORDS <= STAT::frames::WORDS,
    "the last frame runs past STAT's frames section"
);

impl Default for FrameSection {
    fn default() -> Self {
        zeroed()
    }
}

words!(FrameSection, STAT::frames::WORDS,
    present => STAT::frames::present, cnt => STAT::frames::cnt, seconds => STAT::frames::seconds,
    moving => STAT::frames::moving, moving_ms => STAT::frames::moving_ms,
    missed => STAT::frames::missed, overran => STAT::frames::overran,
    long => STAT::frames::long, turns => STAT::frames::turns, gpu_timed => STAT::frames::gpu_timed,
    interval => STAT::frames::interval, busy => STAT::frames::busy,
    tick => STAT::frames::tick, effect => STAT::frames::effect,
    stage => STAT::frames::stage, task => STAT::frames::task,
    perf => STAT::frames::perf, rest => STAT::frames::rest,
    effects_p95 => STAT::frames::effects_p95, gpu_busy => STAT::frames::gpu_busy,
    gpu_busy_kinds => STAT::frames::gpu_busy_kinds,
    phases_p95 => STAT::frames::phases_p95, gpu_spans_p95 => STAT::frames::gpu_spans_p95,
    gpu_spans_cnt => STAT::frames::gpu_spans_cnt,
    kinds => STAT::frames::kinds, wakes => STAT::frames::wakes,
    gpu_px => STAT::frames::gpu_px, ray_sample => STAT::frames::rays,
    first => STAT::frames::first, last => STAT::frames::last,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreamStat {
    pub cnt: f64,
    pub ok: f64,
    pub aborted: f64,
    pub err: f64,
    pub lat_p50: f64,
    pub lat_p95: f64,
    pub reason_cnt: f64,
    /// Each failure reason as (reason, status, count), the commonest first.
    pub reasons: [[f64; REASON_WORDS]; REASONS],
    pub phase_n: f64,
    /// NaN for a phase no run in the window reported.
    pub phase_mean: [f64; TASK_PHASES],
}

words!(StreamStat, STREAM_STAT::WORDS,
    cnt => STREAM_STAT::cnt, ok => STREAM_STAT::ok, aborted => STREAM_STAT::aborted, err => STREAM_STAT::err,
    lat_p50 => STREAM_STAT::lat_p50, lat_p95 => STREAM_STAT::lat_p95, reason_cnt => STREAM_STAT::reason_cnt, reasons => STREAM_STAT::reasons,
    phase_n => STREAM_STAT::phase_n, phase_mean => STREAM_STAT::phase_mean,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TaskSection {
    pub slots: [StreamStat; CATEGORIES],
}

words!(TaskSection, STAT::tasks::WORDS, slots => STAT::tasks::slots);

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
    pub cell_zoom: f64,
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
    cell_zoom => NOW::view::cell_zoom, hit_kind => NOW::view::hit_kind, pick_zoom => NOW::view::pick_zoom, ray_zoom => NOW::view::ray_zoom,
    folded => NOW::view::folded, place_x => NOW::view::place_x, place_y => NOW::view::place_y, place_z => NOW::view::place_z,
    place_zoom => NOW::view::place_zoom, blocked => NOW::view::blocked,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepStat {
    /// Indices into PHASES and STEP_NAMES.
    pub phase: f64,
    pub name: f64,
    pub ms: f64,
    pub cnt: f64,
    pub max_ms: f64,
}

words!(StepStat, STEP_STAT::WORDS,
    phase => STEP_STAT::phase, name => STEP_STAT::name, ms => STEP_STAT::ms, cnt => STEP_STAT::cnt, max_ms => STEP_STAT::max_ms,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepSection {
    pub cnt: f64,
    pub slots: [StepStat; STEP_CAP],
}

words!(StepSection, STAT::steps::WORDS, cnt => STAT::steps::cnt, slots => STAT::steps::slots);

/// The largest and smallest values the frames held since the recording
/// started (perf.json STAT.recording). Each frame turn's figures are folded
/// in by `fold`.
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
    pub body_slots_peak: f64,
    pub terrain_refusals: f64,
}

impl RecordingSection {
    /// Folds in one frame turn's figures, `h`. Each figure keeps its largest
    /// value, except the largest free block, which keeps its smallest. The
    /// heap's refusals are taken as they stand, since the heap's count only
    /// grows, and the refused tiles are summed.
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
            body_slots_peak: max(self.body_slots_peak, h.body_slots_peak),
            terrain_refusals: self.terrain_refusals + h.terrain_refusals,
        };
    }
}

words!(RecordingSection, STAT::recording::WORDS,
    frames => STAT::recording::frames, heap_live_max_bytes => STAT::recording::heap_live_max_bytes,
    heap_run_max_bytes => STAT::recording::heap_run_max_bytes, heap_slot_max_bytes => STAT::recording::heap_slot_max_bytes,
    heap_range_max_bytes => STAT::recording::heap_range_max_bytes,
    heap_largest_free_min_bytes => STAT::recording::heap_largest_free_min_bytes,
    heap_refusals => STAT::recording::heap_refusals, body_slots_peak => STAT::recording::body_slots_peak, terrain_refusals => STAT::recording::terrain_refusals,
);
