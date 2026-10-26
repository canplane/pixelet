// pxl-kernel/profiler/frame.rs

//! A frame's record, and the worst ones kept.

use core::mem::{offset_of, size_of};

use pxl_spec::bake::constant::OBJECT_FINEST;
use xpute_guest::sched::tick::PHASES;

use crate::profiler::spec::{CENSUS_TAGS, FRAME, LIMITS, WORST};
use crate::profiler::word::{words, Words};
use pxl_render::frame::draw::{DrawCensus, DRAW_PIPELINES};
use pxl_render::gpu::api::OBJECT_OPS;
use pxl_render::gpu::span_spec::GPU_SPAN_NAMES;

pub const CENSUS_LEN: usize = CENSUS_TAGS;

/// A tag a draw pipeline, in the renderer's order (pxl-render frame/draw.rs
/// `DRAW_PIPELINES`): a pipeline added without its tag does not compile.
const _: () = assert!(
    CENSUS_LEN == DRAW_PIPELINES.len(),
    "a census tag for every pipeline a draw names"
);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// Lags a frame or two: the read that landed with this frame, where
    /// `read` is 1, its spans and `px` the world's pixels of the frame it
    /// timed, what the spans were measured over; where it is 0, nothing was
    /// measured with this frame and both are 0. The frame's GPU time is its
    /// spans' sum (pxl-render renderer.rs `gpu_ms`).
    pub read: f64,
    pub spans: [f64; GPU_SPANS],
    pub px: f64,
    /// The share of the display's pixels, each way, the world was drawn at.
    pub scale: f64,
    /// Every draw by its pipeline: the frame's draws and triangles are its sums.
    pub census: DrawCensus,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    /// The renderer's upload table, its uploads and their bytes, and its depth
    /// targets' and texture layers' bytes: each as counted (pxl-render
    /// gpu/renderer.rs `FrameStat`).
    pub uploads: f64,
    pub uploads_bytes: f64,
    pub depth_bytes: f64,
    pub pack_bytes: f64,
    /// The GPU objects the renderer holds, by the op that made them, as its
    /// encoder counts them (xpute-gpu encode `Gpu::objects`): op k + 1 in
    /// slot k, as no op is 0.
    pub objects: [f64; OBJECT_OPS - 1],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileSample {
    pub by_zoom: [f64; ZOOMS],
    pub place: f64,
}

/// The heap as its allocator states it (xpute-guest mem/heap.rs `HeapStat`),
/// by its names and in bytes as it counts them: each figure what it is, not
/// a rounding of it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapSample {
    /// The high-water mark: how far the heap has ever reached, never less.
    pub used_bytes: f64,
    /// Pages lent out, a carved run counted whole.
    pub live_bytes: f64,
    /// Of what is lent, what was carved into slots, and what is held in
    /// slots; their difference is what the carving costs.
    pub run_bytes: f64,
    pub slot_bytes: f64,
    pub range_bytes: f64,
    pub largest_free_bytes: f64,
    pub refusals: f64,
}

/// The rings' peaks since the frame before, as the door keeps them
/// (xpute-guest ipc/door.rs `DoorStat`): the completion ring's length, the
/// submission ring's as a drain started, and the most submissions a drain
/// held back for lack of room (each answered a frame late).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingSample {
    pub cq_peak: f64,
    pub sq_peak: f64,
    pub held_peak: f64,
}

/// One frame turn and the interval it opened (profiler.json FRAME): every
/// figure of one turn, landed when the next frame turn closes the interval.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub t: f64,
    /// The host's interval from this frame to the next, which the turns in
    /// it filled: not the world's step, which stops at MAX_STEP_S.
    pub interval_ms: f64,
    /// The display's refresh, and the interval the host paced this frame at,
    /// a whole number of refreshes, as the host granted them (xpute host
    /// sched/quantum.ts `Grant`).
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
    pub phases: [f64; PHASES.len()],
    pub stage_ms: f64,
    pub tasks_ms: f64,
    pub worst_effect_ms: f64,
    pub worst_effect_name: f64,
    /// The renderer's FRAME_KINDS and the edge's WAKES, by index.
    pub kind: f64,
    pub wake: f64,
    pub wake_in_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
    pub tiles: TileSample,
    pub heap: HeapSample,
    pub rings: RingSample,
    /// The most body slots out at once since the frame before
    /// (sys/io/body.rs `take_peak`).
    pub body_slots_peak: f64,
    /// Requests waiting for a credit, and out on one, by stream.
    pub waiting: [f64; STREAMS],
    pub out: [f64; STREAMS],
    pub terrain_refused: f64,
    pub oracle: OracleSample,
}

/// The `ray_oracle` measurement's sums since boot, as the renderer keeps
/// them (pxl-render gpu/ray/cast.rs `Oracle`): the frames whose counts were
/// read back, the pixels the cast stopped, those the walk again stopped
/// otherwise, those it lacked a node for, its visits, the reads that found
/// the node it started at, and those whose walk left that node.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OracleSample {
    pub frames: f64,
    pub stops: f64,
    pub differed: f64,
    pub lacked: f64,
    pub visits: f64,
    pub located: f64,
    pub left: f64,
}

/// Kept with the frame before it, since geometry, textures and heap only mean anything as deltas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    pub has_prev: f64,
    pub prev: FrameSample,
}

words!(FrameSample, FRAME::WORDS,
    t => FRAME::t, interval_ms => FRAME::interval_ms, refresh_ms => FRAME::refresh_ms, paced_ms => FRAME::paced_ms, settled => FRAME::settled,
    turns => FRAME::turns, busy_ms => FRAME::busy_ms, budget_ms => FRAME::budget_ms, effects_ms => FRAME::effects_ms, tick_ms => FRAME::tick_ms,
    phases => FRAME::phases, stage_ms => FRAME::stage_ms, tasks_ms => FRAME::tasks_ms,
    worst_effect_ms => FRAME::worst_effect_ms, worst_effect_name => FRAME::worst_effect_name,
    kind => FRAME::kind_name, wake => FRAME::wake_name, wake_in_ms => FRAME::wake_in_ms,
    gpu.read => FRAME::gpu_read, gpu.spans => FRAME::gpu_spans, gpu.px => FRAME::gpu_px, gpu.scale => FRAME::scale,
    gpu.census => FRAME::census,
    device.uploads => FRAME::uploads, device.uploads_bytes => FRAME::uploads_bytes,
    device.depth_bytes => FRAME::depth_bytes, device.pack_bytes => FRAME::pack_bytes, device.objects => FRAME::gpu_objects,
    tiles.by_zoom => FRAME::zoom_tiles, tiles.place => FRAME::tiles_place,
    heap.used_bytes => FRAME::heap_used_bytes, heap.live_bytes => FRAME::heap_live_bytes, heap.run_bytes => FRAME::heap_run_bytes,
    heap.slot_bytes => FRAME::heap_slot_bytes, heap.range_bytes => FRAME::heap_range_bytes,
    heap.largest_free_bytes => FRAME::heap_largest_free_bytes, heap.refusals => FRAME::heap_refusals,
    rings.cq_peak => FRAME::ring_cq_peak, rings.sq_peak => FRAME::ring_sq_peak, rings.held_peak => FRAME::ring_held_peak,
    body_slots_peak => FRAME::body_slots_peak,
    waiting => FRAME::waiting, out => FRAME::out,
    terrain_refused => FRAME::cap_terrain_refused,
    oracle.frames => FRAME::oracle_frames, oracle.stops => FRAME::oracle_stops,
    oracle.differed => FRAME::oracle_differed, oracle.lacked => FRAME::oracle_lacked,
    oracle.visits => FRAME::oracle_visits, oracle.located => FRAME::oracle_located,
    oracle.left => FRAME::oracle_left,
);

words!(WorstFrame, WORST::WORDS, sample => WORST::sample, has_prev => WORST::has_prev, prev => WORST::prev);

pub const GPU_SPANS: usize = GPU_SPAN_NAMES;
pub const STREAMS: usize = crate::sys::io::demand_spec::STREAM_NAMES;
pub const ZOOMS: usize = LIMITS::ZOOMS as usize;

const _: () = assert!(
    pxl_spec::space::layer::lattice::GROUND.tile_zoom(OBJECT_FINEST) < ZOOMS as i32,
    "a zoom slot for every cell drawn, the finest object tier to the pyramid's top"
);

/// A frame that dropped a vsync while moving: its interval ran past the
/// paced one by half a refresh or more, so the next frame landed a refresh
/// late at least — the host's own test (xpute host sched/quantum.ts
/// `observe`). Still frames take a larger budget on purpose, and a repeated
/// image is no visible stutter.
pub fn missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.interval_ms > f.paced_ms + f.refresh_ms / 2.0
}

/// Past even the hard ceiling: one step too big for the forward-progress waiver.
pub fn overran(f: &FrameSample) -> bool {
    f.tick_ms > f.budget_ms + crate::sys::sched::tick::HARD_SLACK_MS
}

/// What the interval's turns ran past it, read off the interval and the
/// turns rather than kept beside them.
pub fn over_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.interval_ms).max(0.0)
}

/// The interval's turns ran past it: a long frame, and `over_ms` by how much.
pub fn long(f: &FrameSample) -> bool {
    over_ms(f) > 0.0
}
