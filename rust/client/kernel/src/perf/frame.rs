// pxl-kernel/perf/frame.rs

//! A frame's record, and the worst ones kept.

use core::mem::{offset_of, size_of};

use crate::sys::sched::phase_spec::PHASES;
use pxl_engine::tally;

use crate::perf::spec::{FRAME, FRAME_GROUP, WORST};
use crate::perf::word::{as_words, as_words_mut, words, Words};
use crate::sys::sched::tick;
use crate::turn::spec;
use pxl_render::gpu::ray;
use pxl_render::gpu::span_spec::GPU_SPAN_NAMES;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// This frame's own read, written back as it lands a frame or two later
    /// (record.rs `perf_record_gpu`): where `read` is 1, its spans, the
    /// time they ran, those that overlap counted once (`busy_ms`), the
    /// frame's GPU time, which the spans' sum overstates where passes
    /// overlap, and `px` the world's pixels the frame drew, what the spans
    /// were measured over; where it is 0, nothing measured this frame and
    /// all are 0.
    pub read: f64,
    pub spans: [f64; GPU_SPANS],
    pub busy_ms: f64,
    pub px: f64,
    /// The share of the display's pixels, each way, the world was drawn at,
    /// and the camera grid's lattice its beam rays and cells were laid on,
    /// in cells a side (pxl-kernel domain/view/frame/resolution.rs).
    pub scale: f64,
    pub lattice: f64,
    /// The frame's draws and their triangles, every pipeline a draw names
    /// (pxl-render renderer.rs `FrameStat::draws`, summed).
    pub draws: f64,
    pub triangles: f64,
    /// The renderer's number of this frame (pxl-render renderer.rs
    /// `FrameStat::frame`), which a read names as it lands; 0 where it drew
    /// none.
    pub frame: f64,
    /// Why the light's map cast, a bit each in LIGHT_MOVED's order, 0 where
    /// it did not; and the boxes decor's slots were marked by to generate
    /// again (pxl-render renderer.rs `FrameStat`).
    pub light_moved: f64,
    pub decor_boxes: f64,
}

/// The renderer's upload table, its uploads and their bytes, and its
/// texture layers' bytes: each as counted (pxl-render gpu/renderer.rs
/// `FrameStat`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    pub uploads: f64,
    pub uploads_bytes: f64,
    pub pack_bytes: f64,
}

/// One frame turn and the interval it opened (perf.json FRAME): every
/// figure of one turn, landed when the next frame turn closes the interval.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub t: f64,
    /// The host's interval from this frame to the next, which the turns in
    /// it filled: not the world's step, which stops at MAX_STEP_S.
    pub interval_ms: f64,
    /// The display's refresh, and the interval the host paced this frame at,
    /// a whole number of refreshes, as the host granted them (@xpute/host
    /// sched/frame_pacing.ts `FrameGrant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    /// 1 where the host settled the interval, not interactive, as it says
    /// with the frame that closes it: written once, as the interval lands.
    pub settled: f64,
    /// Every turn the interval held, the frame's and those without a host
    /// frame, and their time, rise to fall each.
    pub turns: f64,
    pub busy_ms: f64,
    pub budget_ms: f64,
    /// The parts of `busy_ms`: what is left of it is unmeasured.
    pub effects_ms: f64,
    pub tick_ms: f64,
    pub phases: [f64; PHASES],
    pub stage_ms: f64,
    pub tasks_ms: f64,
    /// What the profiler cost the interval: its own record of the frame and
    /// the shell's reads of it.
    pub perf_ms: f64,
    /// Each effect's time in the frame turn, in EFFECT_NAMES' order.
    pub effects: [f64; EFFECTS],
    /// The renderer's FRAME_KINDS and the edge's WAKES, by index.
    pub kind: f64,
    pub wake: f64,
    pub gpu: GpuSample,
    /// The ray counts of this frame, written back as they land
    /// (record.rs `perf_record_rays`).
    pub rays: RaySample,
    /// The terrain tiles the tile cache took in during the frame turn.
    pub terrain_landed: f64,
    pub device: DeviceSample,
    /// The heap's high-water mark (xpute-guest mem/heap.rs `HeapStat`).
    pub heap_used_bytes: f64,
}

/// The `ray_counts` setting's counts of one frame, as the renderer hands
/// them (pxl-render gpu/ray/tally.rs `RayCounts`), in perf.json FRAME's
/// `rays` group's order: whether the eye cast in full, its pixels and its
/// lanes; whether the light's map cast, its slots and its lanes
/// (pxl-engine `tally`). What an observer took, walked and took the sky
/// for is what it cast again, of its pixels or slots.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RaySample {
    pub eye_cast: f64,
    pub eye_rays: f64,
    pub eye_beam: f64,
    pub eye_sample: f64,
    pub eye_miss: f64,
    pub eye_took: f64,
    pub eye_walked: f64,
    pub eye_sky: f64,
    pub eye_boxes: f64,
    pub light_cast: f64,
    pub light_rays: f64,
    pub light_beam: f64,
    pub light_sample: f64,
    pub light_miss: f64,
    pub light_took: f64,
    pub light_walked: f64,
    pub light_sky: f64,
    pub light_boxes: f64,
}

impl RaySample {
    /// The renderer's counts (`RAY_COUNTS`), in their order.
    pub fn of(c: [f64; ray::RAY_COUNTS]) -> RaySample {
        let at = |o: u32, k: usize| c[ray::count_at(o, k)];
        let lane = |o: u32, l: u32| at(o, ray::LANES_AT + l as usize);
        let (eye, light) = (tally::EYE, tally::LIGHT);
        RaySample {
            eye_cast: at(eye, ray::CAST),
            eye_rays: at(eye, ray::RAYS),
            eye_beam: lane(eye, tally::BEAM),
            eye_sample: lane(eye, tally::SAMPLE),
            eye_miss: lane(eye, tally::MISS),
            eye_took: lane(eye, tally::TOOK),
            eye_walked: lane(eye, tally::WALKED),
            eye_sky: lane(eye, tally::SKY),
            eye_boxes: lane(eye, tally::BOXES),
            light_cast: at(light, ray::CAST),
            light_rays: at(light, ray::RAYS),
            light_beam: lane(light, tally::BEAM),
            light_sample: lane(light, tally::SAMPLE),
            light_miss: lane(light, tally::MISS),
            light_took: lane(light, tally::TOOK),
            light_walked: lane(light, tally::WALKED),
            light_sky: lane(light, tally::SKY),
            light_boxes: lane(light, tally::BOXES),
        }
    }

    /// `o`'s counts added to these, lane by lane.
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
    /// `s`, and what changed from `prev`, the frame before it, where there
    /// was one.
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
    turns => FRAME::turns, busy_ms => FRAME::busy_ms, budget_ms => FRAME::budget_ms, effects_ms => FRAME::effects_ms, tick_ms => FRAME::tick_ms,
    phases => FRAME::phases, stage_ms => FRAME::stage_ms, tasks_ms => FRAME::tasks_ms,
    perf_ms => FRAME::perf_ms, effects => FRAME::effects,
    kind => FRAME::kind_name, wake => FRAME::wake_name,
    gpu.read => FRAME::gpu_read, gpu.spans => FRAME::gpu_spans, gpu.busy_ms => FRAME::gpu_busy_ms, gpu.px => FRAME::gpu_px, gpu.scale => FRAME::scale,
    gpu.lattice => FRAME::lattice,
    gpu.draws => FRAME::draws, gpu.triangles => FRAME::triangles, gpu.frame => FRAME::gpu_frame,
    gpu.light_moved => FRAME::light_moved, gpu.decor_boxes => FRAME::decor_boxes,
    rays => FRAME::eye_cast,
    terrain_landed => FRAME::terrain_landed,
    device.uploads => FRAME::uploads, device.uploads_bytes => FRAME::uploads_bytes, device.pack_bytes => FRAME::pack_bytes,
    heap_used_bytes => FRAME::heap_used_bytes,
);

// The `rays` group's columns, each where FRAME lays it, from the group's first.
words!(RaySample, FRAME_GROUP::rays,
    eye_cast => 0, eye_rays => FRAME::eye_rays - FRAME::eye_cast,
    eye_beam => FRAME::eye_beam - FRAME::eye_cast, eye_sample => FRAME::eye_sample - FRAME::eye_cast,
    eye_miss => FRAME::eye_miss - FRAME::eye_cast, eye_took => FRAME::eye_took - FRAME::eye_cast,
    eye_walked => FRAME::eye_walked - FRAME::eye_cast, eye_sky => FRAME::eye_sky - FRAME::eye_cast,
    eye_boxes => FRAME::eye_boxes - FRAME::eye_cast,
    light_cast => FRAME::light_cast - FRAME::eye_cast, light_rays => FRAME::light_rays - FRAME::eye_cast,
    light_beam => FRAME::light_beam - FRAME::eye_cast,
    light_sample => FRAME::light_sample - FRAME::eye_cast, light_miss => FRAME::light_miss - FRAME::eye_cast,
    light_took => FRAME::light_took - FRAME::eye_cast, light_walked => FRAME::light_walked - FRAME::eye_cast,
    light_sky => FRAME::light_sky - FRAME::eye_cast, light_boxes => FRAME::light_boxes - FRAME::eye_cast,
);

words!(WorstFrame, WORST::WORDS,
    sample => WORST::sample, has_prev => WORST::has_prev, uploads_delta => WORST::uploads_delta,
    uploads_delta_bytes => WORST::uploads_delta_bytes, heap_used_delta_bytes => WORST::heap_used_delta_bytes,
);

pub const GPU_SPANS: usize = GPU_SPAN_NAMES;
pub const EFFECTS: usize = spec::EFFECT_NAMES;

/// A frame that dropped a vsync while moving: its interval ran past the
/// paced one by half a refresh or more, so it landed a refresh late at least.
/// Half a refresh is the jitter the host's pacing allows a frame, skipping
/// one nearer the last turn than the paced interval less that (xpute host
/// sched/strobe.ts `turn`).
pub fn is_missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.interval_ms > f.paced_ms + f.refresh_ms / 2.0
}

/// Past even the hard ceiling: one step too big for the forward-progress waiver.
pub fn overran(f: &FrameSample) -> bool {
    f.tick_ms > f.budget_ms + tick::HARD_SLACK_MS
}

/// What the interval's turns ran past it.
pub fn over_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.interval_ms).max(0.0)
}

/// What the interval's turns spent outside every part measured of them: the
/// door's commands, its pump and the turns' own rise and fall.
pub fn unmeasured_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.effects_ms - f.tick_ms - f.stage_ms - f.tasks_ms - f.perf_ms).max(0.0)
}

/// The interval's turns ran past it: a long frame, and `over_ms` by how much.
pub fn long(f: &FrameSample) -> bool {
    over_ms(f) > 0.0
}
