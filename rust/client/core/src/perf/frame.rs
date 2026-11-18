// pxl-core/perf/frame.rs

//! The record of one frame as the profiler's tables lay it out (perf.json5
//! FRAME), and the record of one of the worst frames (WORST).

use core::mem::{offset_of, size_of};

use crate::spec::phase::Phase;
use pxl_engine::count;

use crate::perf::word::{AsWords, as_words, as_words_mut, words, zeroed};
use crate::spec::perf::{frame, frame_group, worst};
use crate::spec::step::{EffectName, InvalidationReason, StagePartName};
use pxl_render::ray;
use pxl_render::spec::span::GpuSpanName;
use pxl_spec::bake::publication::BaseKind;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GpuSample {
    /// This frame's GPU timings. They land a frame or two later and are
    /// written back into this record (record.rs `perf_record_gpu`). `timed`
    /// is 1 when the frame was measured, and otherwise 0 with every field 0.
    /// `busy_ms` is the frame's GPU time, with overlapping spans counted
    /// once; the sum of `spans` overstates it where passes overlap. `px` is
    /// the number of world pixels the frame drew, which the spans were
    /// measured over.
    pub timed: f64,
    pub spans: [f64; GPU_SPAN_CNT],
    pub busy_ms: f64,
    pub px: f64,
    /// The fraction of the display's resolution, along each axis, at which
    /// the world was drawn (domain/scene/resolution.rs).
    pub scale: f64,
    /// The frame's draws and their triangles, summed over every pipeline
    /// (pxl-render renderer.rs `FrameStat::draw_stat`).
    pub draw_cnt: f64,
    pub triangle_cnt: f64,
    /// The renderer's number for this frame (pxl-render renderer.rs
    /// `FrameStat::frame`). A GPU read names this number when it lands, so
    /// the read finds its frame. It is 0 when the renderer drew no frame.
    pub frame: f64,
    /// What changed that cast the light's map, one bit each in
    /// `LIGHT_CHANGED`'s order, and 0 when it was not cast.
    pub light_changed: f64,
}

impl Default for GpuSample {
    fn default() -> Self {
        zeroed::<FrameSample>().gpu
    }
}

/// The renderer's upload table: how many uploads it holds and their bytes,
/// as the renderer counts them (pxl-render renderer.rs `FrameStat`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct DeviceSample {
    pub upload_cnt: f64,
    pub upload_bytes: f64,
}

/// One frame and the interval it opened (perf.json5 FRAME). The record
/// lands when the next frame closes the interval.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FrameSample {
    pub t: f64,
    /// The interval from this frame to the next. It is not the world's step,
    /// which is capped at `MAX_STEP_S`.
    pub interval_ms: f64,
    /// The display's interval as the frame's pacing takes it
    /// (domain/scene/rate.rs `display_ms`), and the frame's target interval,
    /// which it was paced at (`target_ms`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    /// 1 when the frame asked for no next one.
    pub settled: f64,
    /// The frame's entry's budget, from the entry's start to its end
    /// (sys/sched.rs `budget_ms`), and 1 when the scheduler found it spent
    /// as the tick ended (`spent`).
    pub budget_ms: f64,
    pub overran: f64,
    /// The frame's measured parts: the tick and each of its phases, the
    /// requests' pump, and each part of the stage, indexed by
    /// `StagePartName`.
    pub tick_ms: f64,
    pub phases: [f64; Phase::ALL.len()],
    pub pump_ms: f64,
    pub stage_parts: [f64; STAGE_PART_CNT],
    /// What the ground's fill took in the entry after the frame's (record.rs
    /// `perf_record_fill`).
    pub fill_ms: f64,
    /// What the profiler itself cost the interval: recording the frame, and
    /// the UI's reads of the records.
    pub perf_ms: f64,
    /// The time of each effect in the frame, indexed by `EffectName`.
    pub effects: [f64; EFFECT_CNT],
    /// How many times a frame was asked for since the frame before was
    /// recorded, indexed by `InvalidationReason`.
    pub invalidations: [f64; INVALIDATION_REASON_CNT],
    /// The frame's kind, its renderer's `FrameKind`'s number.
    pub kind: f64,
    pub gpu: GpuSample,
    /// This frame's ray counts, written back when they land (record.rs
    /// `perf_record_rays`).
    pub ray_sample: RaySample,
    /// The tiles each base layer's tile cache took in during the frame,
    /// indexed by `BaseKind`.
    pub loads: [f64; LAYER_CNT],
    /// The bricks the ground's fill filled after the frame.
    pub fill_brick_cnt: f64,
    pub device: DeviceSample,
    /// The heap's high-water mark (sys/heap.rs `HeapStat`).
    pub heap_used_bytes: f64,
}

/// One frame's ray counts, which the renderer gathers while the `ray_cnt`
/// setting is on (pxl-render ray/count.rs `RayCount`). The fields
/// follow the order of perf.json5 FRAME's `rays` group. For the eye they say
/// whether it cast in full, how many pixels it cast, and its lane counts.
/// For the light's map they say whether it cast, how many slots it cast,
/// and its lane counts (pxl-engine `count`). The voxel, walk and sky
/// lanes divide up what an observer cast again of its pixels or slots.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RaySample {
    pub eye_cast: f64,
    pub eye_ray_cnt: f64,
    pub eye_beam: f64,
    pub eye_sample: f64,
    pub eye_hole_walk: f64,
    pub eye_ask_cnt: f64,
    pub eye_voxel: f64,
    pub eye_hole: f64,
    pub eye_sky: f64,
    pub eye_box_cnt: f64,
    pub light_cast: f64,
    pub light_ray_cnt: f64,
    pub light_beam: f64,
    pub light_sample: f64,
    pub light_hole_walk: f64,
    pub light_ask_cnt: f64,
    pub light_voxel: f64,
    pub light_hole: f64,
    pub light_sky: f64,
    pub light_box_cnt: f64,
}

impl RaySample {
    /// Reads the renderer's counts, laid out as `ray::cnt_at` places them.
    pub(crate) fn of(c: [f64; ray::RAY_CNT_CNT]) -> RaySample {
        let at = |o: u32, k: usize| c[ray::cnt_at(o, k)];
        let lane = |o: u32, l: u32| at(o, ray::LANES_AT + l as usize);
        let (eye, light) = (count::EYE, count::LIGHT);
        RaySample {
            eye_cast: at(eye, ray::CAST),
            eye_ray_cnt: at(eye, ray::RAYS),
            eye_beam: lane(eye, count::BEAM),
            eye_sample: lane(eye, count::SAMPLE),
            eye_hole_walk: lane(eye, count::HOLE_WALK),
            eye_ask_cnt: lane(eye, count::ASK),
            eye_voxel: lane(eye, count::VOXEL),
            eye_hole: lane(eye, count::HOLE),
            eye_sky: lane(eye, count::SKY),
            eye_box_cnt: lane(eye, count::BOX),
            light_cast: at(light, ray::CAST),
            light_ray_cnt: at(light, ray::RAYS),
            light_beam: lane(light, count::BEAM),
            light_sample: lane(light, count::SAMPLE),
            light_hole_walk: lane(light, count::HOLE_WALK),
            light_ask_cnt: lane(light, count::ASK),
            light_voxel: lane(light, count::VOXEL),
            light_hole: lane(light, count::HOLE),
            light_sky: lane(light, count::SKY),
            light_box_cnt: lane(light, count::BOX),
        }
    }

    pub(crate) fn add(&mut self, o: &RaySample) {
        let (to, from) = (as_words_mut(self), as_words(o));
        for (t, f) in to.iter_mut().zip(from) {
            *t += f;
        }
    }
}

/// One of the worst frames, and what changed from the frame before it
/// (perf.json5 WORST).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct WorstFrame {
    pub sample: FrameSample,
    pub has_prev: f64,
    pub upload_cnt_delta: f64,
    pub upload_delta_bytes: f64,
    pub heap_used_delta_bytes: f64,
}

impl WorstFrame {
    /// The worst frame `s`, with what changed since `prev`, the frame before
    /// it, when there was one.
    pub(crate) fn of(s: FrameSample, prev: Option<&FrameSample>) -> WorstFrame {
        let Some(p) = prev else {
            return WorstFrame {
                sample: s,
                ..WorstFrame::default()
            };
        };
        WorstFrame {
            sample: s,
            has_prev: 1.0,
            upload_cnt_delta: s.device.upload_cnt - p.device.upload_cnt,
            upload_delta_bytes: s.device.upload_bytes - p.device.upload_bytes,
            heap_used_delta_bytes: s.heap_used_bytes - p.heap_used_bytes,
        }
    }
}

words!(FrameSample, frame::WORDS,
    t => frame::T, interval_ms => frame::INTERVAL_MS, refresh_ms => frame::REFRESH_MS, paced_ms => frame::PACED_MS, settled => frame::SETTLED,
    budget_ms => frame::BUDGET_MS, overran => frame::OVERRAN, tick_ms => frame::TICK_MS,
    phases => frame::PHASES, pump_ms => frame::PUMP_MS, stage_parts => frame::STAGE_PARTS, fill_ms => frame::FILL_MS,
    perf_ms => frame::PERF_MS, effects => frame::EFFECTS, invalidations => frame::INVALIDATIONS,
    kind => frame::KIND_NAME,
    gpu.timed => frame::GPU_TIMED, gpu.spans => frame::GPU_SPANS, gpu.busy_ms => frame::GPU_BUSY_MS, gpu.px => frame::GPU_PX, gpu.scale => frame::SCALE,
    gpu.draw_cnt => frame::DRAW_CNT, gpu.triangle_cnt => frame::TRIANGLE_CNT, gpu.frame => frame::GPU_FRAME,
    gpu.light_changed => frame::LIGHT_CHANGED,
    ray_sample => frame::EYE_CAST,
    loads => frame::LOADS, fill_brick_cnt => frame::FILL_BRICK_CNT,
    device.upload_cnt => frame::UPLOAD_CNT, device.upload_bytes => frame::UPLOAD_BYTES,
    heap_used_bytes => frame::HEAP_USED_BYTES,
);

// The `rays` group's columns, each at its offset in FRAME counted from the
// group's first column.
words!(RaySample, frame_group::RAYS.len(),
    eye_cast => 0, eye_ray_cnt => frame::EYE_RAY_CNT - frame::EYE_CAST,
    eye_beam => frame::EYE_BEAM - frame::EYE_CAST, eye_sample => frame::EYE_SAMPLE - frame::EYE_CAST,
    eye_hole_walk => frame::EYE_HOLE_WALK - frame::EYE_CAST, eye_ask_cnt => frame::EYE_ASK_CNT - frame::EYE_CAST, eye_voxel => frame::EYE_VOXEL - frame::EYE_CAST,
    eye_hole => frame::EYE_HOLE - frame::EYE_CAST, eye_sky => frame::EYE_SKY - frame::EYE_CAST,
    eye_box_cnt => frame::EYE_BOX_CNT - frame::EYE_CAST,
    light_cast => frame::LIGHT_CAST - frame::EYE_CAST, light_ray_cnt => frame::LIGHT_RAY_CNT - frame::EYE_CAST,
    light_beam => frame::LIGHT_BEAM - frame::EYE_CAST,
    light_sample => frame::LIGHT_SAMPLE - frame::EYE_CAST, light_hole_walk => frame::LIGHT_HOLE_WALK - frame::EYE_CAST, light_ask_cnt => frame::LIGHT_ASK_CNT - frame::EYE_CAST,
    light_voxel => frame::LIGHT_VOXEL - frame::EYE_CAST, light_hole => frame::LIGHT_HOLE - frame::EYE_CAST,
    light_sky => frame::LIGHT_SKY - frame::EYE_CAST, light_box_cnt => frame::LIGHT_BOX_CNT - frame::EYE_CAST,
);

words!(WorstFrame, worst::WORDS,
    sample => worst::SAMPLE, has_prev => worst::HAS_PREV, upload_cnt_delta => worst::UPLOAD_CNT_DELTA,
    upload_delta_bytes => worst::UPLOAD_DELTA_BYTES, heap_used_delta_bytes => worst::HEAP_USED_DELTA_BYTES,
);

pub(crate) const GPU_SPAN_CNT: usize = GpuSpanName::ALL.len();
pub(crate) const EFFECT_CNT: usize = EffectName::ALL.len();
pub(crate) const STAGE_PART_CNT: usize = StagePartName::ALL.len();
pub(crate) const INVALIDATION_REASON_CNT: usize = InvalidationReason::ALL.len();
pub(crate) const LAYER_CNT: usize = BaseKind::ALL.len();

impl FrameSample {
    /// The effects' time, the sum of each effect's.
    pub(crate) fn effects_ms(&self) -> f64 {
        self.effects.iter().sum()
    }

    /// The stage's time, the sum of its parts'.
    pub(crate) fn stage_ms(&self) -> f64 {
        self.stage_parts.iter().sum()
    }
}
