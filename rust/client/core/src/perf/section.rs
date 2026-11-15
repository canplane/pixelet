// pxl-core/perf/section.rs

//! Every STAT and NOW section of perf.json5 as a record, each held to its
//! layout by `words!`.

use core::mem::{offset_of, size_of};

use crate::perf::frame::{EFFECT_CNT, FrameSample, GPU_SPAN_CNT, RaySample};
use crate::perf::word::{AsWords, words, zeroed};
use crate::spec::demand::{StreamName, TaskPhaseName};
use crate::spec::perf::{frame, limit, now, percentile, stat, step_stat, stream_stat};
use crate::spec::phase::Phase;
use pxl_render::spec::span::FrameKind;

const REASON_CAP: usize = limit::REASON_CAP as usize;
const REASON_WORDS: usize = limit::REASON_WORDS as usize;
const STEP_CAP: usize = limit::STEP_CAP as usize;
pub(crate) const TASK_PHASE_CNT: usize = TaskPhaseName::ALL.len();
const CATEGORY_CNT: usize = StreamName::ALL.len();

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Percentile {
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
}

words!(Percentile, percentile::WORDS, p50 => percentile::P50, p95 => percentile::P95, max => percentile::MAX);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RecordingStateSection {
    pub recording: f64,
    pub elapsed_ms: f64,
    pub truncated: f64,
    pub frame_cnt: f64,
    pub task_cnt: f64,
    pub worst_cnt: f64,
}

words!(RecordingStateSection, now::recording_state::WORDS,
    recording => now::recording_state::RECORDING, elapsed_ms => now::recording_state::ELAPSED_MS, truncated => now::recording_state::TRUNCATED,
    frame_cnt => now::recording_state::FRAME_CNT, task_cnt => now::recording_state::TASK_CNT, worst_cnt => now::recording_state::WORST_CNT,
);

/// Whether the core draws, as perf.json5's NOW `draw` says.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DrawSection {
    pub frame_cnt: f64,
    pub refused_admit: f64,
    pub refused_stream: f64,
    pub generation: f64,
    pub status: f64,
    pub err_cnt: f64,
    pub unfinished: f64,
}

words!(DrawSection, now::draw::WORDS,
    frame_cnt => now::draw::FRAME_CNT, refused_admit => now::draw::REFUSED_ADMIT, refused_stream => now::draw::REFUSED_STREAM,
    generation => now::draw::GENERATION, status => now::draw::STATUS, err_cnt => now::draw::ERR_CNT, unfinished => now::draw::UNFINISHED,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrameSection {
    /// 0 for a window with no frames, where every other word is 0 too.
    pub present: f64,
    pub cnt: f64,
    pub seconds: f64,
    pub moving: f64,
    /// The sum of the moving frames' intervals. Divided by `moving`, it gives
    /// their mean interval, and a second over that mean is the rate they were
    /// drawn at.
    pub moving_ms: f64,
    pub overran: f64,
    /// How many frames a GPU read timed. The GPU's figures cover these frames
    /// alone.
    pub gpu_timed: f64,
    pub interval: Percentile,
    pub tick: Percentile,
    pub effect: Percentile,
    pub stage: Percentile,
    pub perf: Percentile,
    /// Each effect's p95, indexed by `EffectName`.
    pub effects_p95: [f64; EFFECT_CNT],
    /// The GPU's time, with overlapping spans counted once.
    pub gpu_busy: Percentile,
    /// Three words for each of the renderer's `FrameKind`s: the p50 and p95 of
    /// the GPU's busy time over that kind's timed frames, and how many there
    /// were. All three are 0 for a kind no timed frame had.
    pub gpu_busy_kinds: [f64; 3 * FrameKind::ALL.len()],
    /// Only the p95 of each phase and each GPU span is kept, because only it
    /// is read. A span's p95 covers the timed frames that ran the span, and
    /// `gpu_spans_cnt` counts those frames.
    pub phases_p95: [f64; Phase::ALL.len()],
    pub gpu_spans_p95: [f64; GPU_SPAN_CNT],
    pub gpu_spans_cnt: [f64; GPU_SPAN_CNT],
    /// How many frames fell in each of the renderer's `FrameKind`s.
    pub kinds: [f64; FrameKind::ALL.len()],
    /// The world's pixel count in the last frame a read timed.
    pub gpu_px: f64,
    /// The ray counts of the window's frames, summed field by field.
    pub ray_sample: RaySample,
    /// The window's first frame and its last.
    pub first: FrameSample,
    pub last: FrameSample,
}

const _: () = assert!(
    stat::frames::LAST + frame::WORDS <= stat::frames::WORDS,
    "the last frame runs past STAT's frames section"
);

impl Default for FrameSection {
    fn default() -> Self {
        zeroed()
    }
}

words!(FrameSection, stat::frames::WORDS,
    present => stat::frames::PRESENT, cnt => stat::frames::CNT, seconds => stat::frames::SECONDS,
    moving => stat::frames::MOVING, moving_ms => stat::frames::MOVING_MS,
    overran => stat::frames::OVERRAN, gpu_timed => stat::frames::GPU_TIMED,
    interval => stat::frames::INTERVAL,
    tick => stat::frames::TICK, effect => stat::frames::EFFECT,
    stage => stat::frames::STAGE,
    perf => stat::frames::PERF,
    effects_p95 => stat::frames::EFFECTS_P95, gpu_busy => stat::frames::GPU_BUSY,
    gpu_busy_kinds => stat::frames::GPU_BUSY_KINDS,
    phases_p95 => stat::frames::PHASES_P95, gpu_spans_p95 => stat::frames::GPU_SPANS_P95,
    gpu_spans_cnt => stat::frames::GPU_SPANS_CNT,
    kinds => stat::frames::KINDS,
    gpu_px => stat::frames::GPU_PX, ray_sample => stat::frames::RAYS,
    first => stat::frames::FIRST, last => stat::frames::LAST,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StreamStat {
    pub cnt: f64,
    pub ok: f64,
    pub aborted: f64,
    pub err: f64,
    pub lat_p50: f64,
    pub lat_p95: f64,
    pub reason_cnt: f64,
    /// Each failure reason as (reason, status, count), the commonest first.
    pub reasons: [[f64; REASON_WORDS]; REASON_CAP],
    pub phase_n: f64,
    /// NaN for a phase no run in the window reported.
    pub phase_mean: [f64; TASK_PHASE_CNT],
}

words!(StreamStat, stream_stat::WORDS,
    cnt => stream_stat::CNT, ok => stream_stat::OK, aborted => stream_stat::ABORTED, err => stream_stat::ERR,
    lat_p50 => stream_stat::LAT_P50, lat_p95 => stream_stat::LAT_P95, reason_cnt => stream_stat::REASON_CNT, reasons => stream_stat::REASONS,
    phase_n => stream_stat::PHASE_N, phase_mean => stream_stat::PHASE_MEAN,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TaskSection {
    pub slots: [StreamStat; CATEGORY_CNT],
}

words!(TaskSection, stat::tasks::WORDS, slots => stat::tasks::SLOTS);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ViewSection {
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

words!(ViewSection, now::view::WORDS,
    eye_x => now::view::EYE_X, eye_y => now::view::EYE_Y, eye_z => now::view::EYE_Z,
    dist => now::view::DIST, polar => now::view::POLAR, azimuth => now::view::AZIMUTH,
    pivot_x => now::view::PIVOT_X, pivot_y => now::view::PIVOT_Y, pivot_z => now::view::PIVOT_Z,
    pivot_lon => now::view::PIVOT_LON, pivot_lat => now::view::PIVOT_LAT, pivot_alt => now::view::PIVOT_ALT,
    hover_x => now::view::HOVER_X, hover_y => now::view::HOVER_Y, hover_z => now::view::HOVER_Z,
    cell_zoom => now::view::CELL_ZOOM, hit_kind => now::view::HIT_KIND, pick_zoom => now::view::PICK_ZOOM, ray_zoom => now::view::RAY_ZOOM,
    folded => now::view::FOLDED, place_x => now::view::PLACE_X, place_y => now::view::PLACE_Y, place_z => now::view::PLACE_Z,
    place_zoom => now::view::PLACE_ZOOM, blocked => now::view::BLOCKED,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StepStat {
    /// Indices into `Phase::ALL` and `StepName::ALL`.
    pub phase: f64,
    pub name: f64,
    pub ms: f64,
    pub cnt: f64,
    pub max_ms: f64,
}

words!(StepStat, step_stat::WORDS,
    phase => step_stat::PHASE, name => step_stat::NAME, ms => step_stat::MS, cnt => step_stat::CNT, max_ms => step_stat::MAX_MS,
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StepSection {
    pub cnt: f64,
    pub slots: [StepStat; STEP_CAP],
}

words!(StepSection, stat::steps::WORDS, cnt => stat::steps::CNT, slots => stat::steps::SLOTS);

/// The largest and smallest values the frames held since the recording
/// started (perf.json5 STAT.recording). Each frame's figures are folded
/// in by `fold`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RecordingSection {
    pub frame_cnt: f64,
    pub heap_live_max_bytes: f64,
    pub heap_run_max_bytes: f64,
    pub heap_slot_max_bytes: f64,
    pub heap_range_max_bytes: f64,
    pub heap_largest_free_min_bytes: f64,
    pub heap_refusal_cnt: f64,
    pub body_slots_peak: f64,
    pub terrain_refusal_cnt: f64,
}

impl RecordingSection {
    /// Folds in one frame's figures, `h`. Each figure keeps its largest
    /// value, except the largest free block, which keeps its smallest. The
    /// heap's refusals are taken as they stand, since the heap's count only
    /// grows, and the refused tiles are summed.
    pub(crate) fn fold(&mut self, h: &RecordingSection) {
        let first = self.frame_cnt == 0.0;
        let max = |a: f64, b: f64| if first { b } else { a.max(b) };
        *self = RecordingSection {
            frame_cnt: self.frame_cnt + 1.0,
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
            heap_refusal_cnt: h.heap_refusal_cnt,
            body_slots_peak: max(self.body_slots_peak, h.body_slots_peak),
            terrain_refusal_cnt: self.terrain_refusal_cnt + h.terrain_refusal_cnt,
        };
    }
}

words!(RecordingSection, stat::recording::WORDS,
    frame_cnt => stat::recording::FRAME_CNT, heap_live_max_bytes => stat::recording::HEAP_LIVE_MAX_BYTES,
    heap_run_max_bytes => stat::recording::HEAP_RUN_MAX_BYTES, heap_slot_max_bytes => stat::recording::HEAP_SLOT_MAX_BYTES,
    heap_range_max_bytes => stat::recording::HEAP_RANGE_MAX_BYTES,
    heap_largest_free_min_bytes => stat::recording::HEAP_LARGEST_FREE_MIN_BYTES,
    heap_refusal_cnt => stat::recording::HEAP_REFUSAL_CNT, body_slots_peak => stat::recording::BODY_SLOTS_PEAK, terrain_refusal_cnt => stat::recording::TERRAIN_REFUSAL_CNT,
);
