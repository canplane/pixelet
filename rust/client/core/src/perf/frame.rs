// pxl-core/perf/frame.rs

//! The record of one frame as the profiler's rings lay it out (perf.json
//! FRAME), and the record of one of the worst frames (WORST).

use core::mem::{offset_of, size_of};

use crate::sys::sched::phase_spec::PHASES;
use pxl_engine::count;

use crate::perf::spec::{FRAME, FRAME_GROUP, WORST};
use crate::perf::word::{as_words, as_words_mut, words, zeroed, AsWords};
use crate::sys::sched::param;
use crate::turn::spec;
use pxl_render::gpu::ray;
use pxl_render::gpu::span_spec::GPU_SPAN_NAMES;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuSample {
    /// This frame's GPU timings. They land a frame or two later and are
    /// written back into this record (record.rs `perf_record_gpu`). `timed`
    /// is 1 when the frame was measured, and otherwise 0 with every field 0.
    /// `busy_ms` is the frame's GPU time, with overlapping spans counted
    /// once; the sum of `spans` overstates it where passes overlap. `px` is
    /// the number of world pixels the frame drew, which the spans were
    /// measured over.
    pub timed: f64,
    pub spans: [f64; GPU_SPANS],
    pub busy_ms: f64,
    pub px: f64,
    /// The fraction of the display's resolution, along each axis, at which
    /// the world was drawn (domain/scene/resolution.rs).
    pub scale: f64,
    /// The frame's draws and their triangles, summed over every pipeline
    /// (pxl-render renderer.rs `FrameStat::draws`).
    pub draws: f64,
    pub triangles: f64,
    /// The renderer's number for this frame (pxl-render renderer.rs
    /// `FrameStat::frame`). A GPU read names this number when it lands, so
    /// the read finds its frame. It is 0 when the renderer drew no frame.
    pub frame: f64,
    /// What changed that cast the light's map, one bit each in LIGHT_CHANGED's
    /// order, and 0 when it was not cast. `decor_rects` counts the rects that
    /// marked decor's slots to be generated again (pxl-render renderer.rs
    /// `FrameStat`).
    pub light_changed: f64,
    pub decor_rects: f64,
}

impl Default for GpuSample {
    fn default() -> Self {
        zeroed::<FrameSample>().gpu
    }
}

/// The renderer's upload table: how many uploads it holds, their bytes, and
/// the bytes of its texture layers, as the renderer counts them (pxl-render
/// gpu/renderer.rs `FrameStat`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    pub uploads: f64,
    pub uploads_bytes: f64,
    pub pack_bytes: f64,
}

/// One frame turn and the interval it opened (perf.json FRAME). The record
/// lands when the next frame turn closes the interval.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub t: f64,
    /// The runtime's interval from this frame to the next, which the turns in
    /// it filled. It is not the world's step, which is capped at MAX_STEP_S.
    pub interval_ms: f64,
    /// The display's refresh interval, and the interval the runtime paced
    /// this frame at, a whole number of refreshes, as the runtime granted
    /// them (@xpute/runtime sched/frame_pacing.ts `FrameGrant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    /// 1 when the runtime ran the interval settled rather than interactive.
    /// The runtime says so with the frame that closes the interval, so this
    /// is written once, when the interval lands.
    pub settled: f64,
    /// How many turns the interval held, the frame's and those without a
    /// runtime frame, and their total time (`busy_ms`), each turn measured
    /// from its rise to its fall.
    pub turns: f64,
    pub busy_ms: f64,
    /// How far the interval's turns ran past its end (`over_ms`).
    pub over_ms: f64,
    pub budget_ms: f64,
    /// The measured parts of `busy_ms`. The rest of it is unmeasured
    /// (`unmeasured_ms`).
    pub effects_ms: f64,
    pub tick_ms: f64,
    pub phases: [f64; PHASES],
    pub stage_ms: f64,
    pub tasks_ms: f64,
    /// What the profiler itself cost the interval: recording the frame, and
    /// the UI's reads of the records.
    pub perf_ms: f64,
    /// The time of each effect in the frame turn, indexed by `EffectName`.
    pub effects: [f64; EFFECTS],
    /// The frame's kind, an index into the renderer's FRAME_KINDS, and the
    /// wake the turn asked for, an index into the edge's WAKES.
    pub kind: f64,
    pub wake: f64,
    pub gpu: GpuSample,
    /// This frame's ray counts, written back when they land (record.rs
    /// `perf_record_rays`).
    pub ray_sample: RaySample,
    /// The terrain tiles the tile cache took in during the frame turn.
    pub terrain_loads: f64,
    pub device: DeviceSample,
    /// The heap's high-water mark (xpute-core mem/heap.rs `HeapStat`).
    pub heap_used_bytes: f64,
}

/// One frame's ray counts, which the renderer gathers while the `ray_cnts`
/// setting is on (pxl-render gpu/ray/count.rs `RayCount`). The fields
/// follow the order of perf.json FRAME's `rays` group. For the eye they say
/// whether it cast in full, how many pixels it cast, and its lane counts.
/// For the light's map they say whether it cast, how many slots it cast,
/// and its lane counts (pxl-engine `count`). The voxel, walk and sky
/// lanes divide up what an observer cast again of its pixels or slots.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaySample {
    pub eye_cast: f64,
    pub eye_rays: f64,
    pub eye_beam: f64,
    pub eye_sample: f64,
    pub eye_hole_walk: f64,
    pub eye_asks: f64,
    pub eye_voxel: f64,
    pub eye_hole: f64,
    pub eye_sky: f64,
    pub eye_boxes: f64,
    pub light_cast: f64,
    pub light_rays: f64,
    pub light_beam: f64,
    pub light_sample: f64,
    pub light_hole_walk: f64,
    pub light_asks: f64,
    pub light_voxel: f64,
    pub light_hole: f64,
    pub light_sky: f64,
    pub light_boxes: f64,
}

impl RaySample {
    /// Reads the renderer's counts, laid out in `RAY_CNTS` order.
    pub fn of(c: [f64; ray::RAY_CNTS]) -> RaySample {
        let at = |o: u32, k: usize| c[ray::cnt_at(o, k)];
        let lane = |o: u32, l: u32| at(o, ray::LANES_AT + l as usize);
        let (eye, light) = (count::EYE, count::LIGHT);
        RaySample {
            eye_cast: at(eye, ray::CAST),
            eye_rays: at(eye, ray::RAYS),
            eye_beam: lane(eye, count::BEAM),
            eye_sample: lane(eye, count::SAMPLE),
            eye_hole_walk: lane(eye, count::HOLE_WALK),
            eye_asks: lane(eye, count::ASKS),
            eye_voxel: lane(eye, count::VOXEL),
            eye_hole: lane(eye, count::HOLE),
            eye_sky: lane(eye, count::SKY),
            eye_boxes: lane(eye, count::BOXES),
            light_cast: at(light, ray::CAST),
            light_rays: at(light, ray::RAYS),
            light_beam: lane(light, count::BEAM),
            light_sample: lane(light, count::SAMPLE),
            light_hole_walk: lane(light, count::HOLE_WALK),
            light_asks: lane(light, count::ASKS),
            light_voxel: lane(light, count::VOXEL),
            light_hole: lane(light, count::HOLE),
            light_sky: lane(light, count::SKY),
            light_boxes: lane(light, count::BOXES),
        }
    }

    pub fn add(&mut self, o: &RaySample) {
        let (to, from) = (as_words_mut(self), as_words(o));
        for (t, f) in to.iter_mut().zip(from) {
            *t += f;
        }
    }
}

/// One of the worst frames, and what changed from the frame before it
/// (perf.json WORST).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    pub has_prev: f64,
    pub uploads_delta: f64,
    pub uploads_delta_bytes: f64,
    pub heap_used_delta_bytes: f64,
}

impl WorstFrame {
    /// The worst frame `s`, with what changed since `prev`, the frame before
    /// it, when there was one.
    pub fn of(s: FrameSample, prev: Option<&FrameSample>) -> WorstFrame {
        let Some(p) = prev else {
            return WorstFrame {
                sample: s,
                ..WorstFrame::default()
            };
        };
        WorstFrame {
            sample: s,
            has_prev: 1.0,
            uploads_delta: s.device.uploads - p.device.uploads,
            uploads_delta_bytes: s.device.uploads_bytes - p.device.uploads_bytes,
            heap_used_delta_bytes: s.heap_used_bytes - p.heap_used_bytes,
        }
    }
}

words!(FrameSample, FRAME::WORDS,
    t => FRAME::t, interval_ms => FRAME::interval_ms, refresh_ms => FRAME::refresh_ms, paced_ms => FRAME::paced_ms, settled => FRAME::settled,
    turns => FRAME::turns, busy_ms => FRAME::busy_ms, over_ms => FRAME::over_ms, budget_ms => FRAME::budget_ms, effects_ms => FRAME::effects_ms, tick_ms => FRAME::tick_ms,
    phases => FRAME::phases, stage_ms => FRAME::stage_ms, tasks_ms => FRAME::tasks_ms,
    perf_ms => FRAME::perf_ms, effects => FRAME::effects,
    kind => FRAME::kind_name, wake => FRAME::wake_name,
    gpu.timed => FRAME::gpu_timed, gpu.spans => FRAME::gpu_spans, gpu.busy_ms => FRAME::gpu_busy_ms, gpu.px => FRAME::gpu_px, gpu.scale => FRAME::scale,
    gpu.draws => FRAME::draws, gpu.triangles => FRAME::triangles, gpu.frame => FRAME::gpu_frame,
    gpu.light_changed => FRAME::light_changed, gpu.decor_rects => FRAME::decor_rects,
    ray_sample => FRAME::eye_cast,
    terrain_loads => FRAME::terrain_loads,
    device.uploads => FRAME::uploads, device.uploads_bytes => FRAME::uploads_bytes, device.pack_bytes => FRAME::pack_bytes,
    heap_used_bytes => FRAME::heap_used_bytes,
);

// The `rays` group's columns, each at its offset in FRAME counted from the
// group's first column.
words!(RaySample, FRAME_GROUP::rays,
    eye_cast => 0, eye_rays => FRAME::eye_rays - FRAME::eye_cast,
    eye_beam => FRAME::eye_beam - FRAME::eye_cast, eye_sample => FRAME::eye_sample - FRAME::eye_cast,
    eye_hole_walk => FRAME::eye_hole_walk - FRAME::eye_cast, eye_asks => FRAME::eye_asks - FRAME::eye_cast, eye_voxel => FRAME::eye_voxel - FRAME::eye_cast,
    eye_hole => FRAME::eye_hole - FRAME::eye_cast, eye_sky => FRAME::eye_sky - FRAME::eye_cast,
    eye_boxes => FRAME::eye_boxes - FRAME::eye_cast,
    light_cast => FRAME::light_cast - FRAME::eye_cast, light_rays => FRAME::light_rays - FRAME::eye_cast,
    light_beam => FRAME::light_beam - FRAME::eye_cast,
    light_sample => FRAME::light_sample - FRAME::eye_cast, light_hole_walk => FRAME::light_hole_walk - FRAME::eye_cast, light_asks => FRAME::light_asks - FRAME::eye_cast,
    light_voxel => FRAME::light_voxel - FRAME::eye_cast, light_hole => FRAME::light_hole - FRAME::eye_cast,
    light_sky => FRAME::light_sky - FRAME::eye_cast, light_boxes => FRAME::light_boxes - FRAME::eye_cast,
);

words!(WorstFrame, WORST::WORDS,
    sample => WORST::sample, has_prev => WORST::has_prev, uploads_delta => WORST::uploads_delta,
    uploads_delta_bytes => WORST::uploads_delta_bytes, heap_used_delta_bytes => WORST::heap_used_delta_bytes,
);

pub const GPU_SPANS: usize = GPU_SPAN_NAMES;
pub const EFFECTS: usize = spec::EFFECT_NAMES;

/// Whether a frame dropped a vsync while moving. Its interval ran past the
/// paced one by more than half a refresh, so it landed at least a refresh
/// late. Half a refresh is the jitter the runtime's pacing allows: the
/// runtime skips a frame that comes sooner after the last turn than the
/// paced interval less half a refresh (xpute runtime sched/scheduler.ts
/// `turn`).
pub fn is_missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.interval_ms > f.paced_ms + f.refresh_ms / 2.0
}

/// Whether the tick ran past even the hard ceiling, the budget plus
/// `HARD_SLACK_MS`. A step that big is too big for the waiver that lets a
/// step overrun the budget so the tick keeps making progress.
pub fn overran(f: &FrameSample) -> bool {
    f.tick_ms > f.budget_ms + param::HARD_SLACK_MS
}

/// How far the interval's turns ran past the interval's end, in
/// milliseconds.
pub fn over_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.interval_ms).max(0.0)
}

/// The time the interval's turns spent outside every measured part: the
/// door's commands, its pump, and each turn's own rise and fall.
pub fn unmeasured_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.effects_ms - f.tick_ms - f.stage_ms - f.tasks_ms - f.perf_ms).max(0.0)
}

/// Whether the frame is long: its interval's turns ran past the interval's
/// end. `over_ms` says by how much.
pub fn long(f: &FrameSample) -> bool {
    f.over_ms > 0.0
}
