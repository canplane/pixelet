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

pub const CENSUS_LEN: usize = CENSUS_TAGS.len();

/// A tag a draw pipeline, in the renderer's order (pxl-render frame/draw.rs
/// `DRAW_PIPELINES`): a pipeline added without its tag does not compile.
const _: () = assert!(CENSUS_LEN == DRAW_PIPELINES.len(), "a census tag for every pipeline a draw names");

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// Lags a frame or two; 0 before the renderer exists or without GPU timing.
    pub ms: f64,
    pub spans: [f64; GPU_SPANS],
    /// Every draw by its pipeline: the frame's draws and triangles are its sums.
    pub census: DrawCensus,
    pub px: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    pub buffers: f64,
    pub buffer_mb: f64,
    pub depth_mb: f64,
    pub layers_mb: f64,
    /// The GPU objects the renderer holds, by the op that made them, as its
    /// encoder counts them (xpute-gpu encode `Gpu::objects`).
    pub objects: [f64; OBJECT_OPS],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileSample {
    pub by_zoom: [f64; ZOOMS],
    pub place: f64,
}

/// The heap as its allocator states it (xpute-guest mem/heap.rs `HeapStat`),
/// in bytes as it counts them: each figure what it is, not a rounding of it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapSample {
    /// The high-water mark: how far the heap has ever reached, never less.
    pub peak_bytes: f64,
    /// Pages lent out, a carved run counted whole.
    pub lent_bytes: f64,
    /// Of what is lent, what was carved into slots, and what is held in
    /// slots; their difference is what the carving costs.
    pub run_bytes: f64,
    pub held_bytes: f64,
    pub range_bytes: f64,
    pub largest_free_bytes: f64,
    pub refusals: f64,
    pub releases: f64,
}

/// The completion ring's peak, and the most submissions a turn held back for
/// lack of room (each answered a frame late).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingSample {
    pub cq_peak: f64,
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
    pub display_ms: f64,
    /// 1 where the host settled the interval, not interactive, as it says
    /// with the frame that closes it: written once, as the interval lands.
    pub settled: f64,
    /// Every turn the interval held, the frame's and those without a host
    /// frame, and their time, rise to fall each.
    pub turns: f64,
    pub busy_ms: f64,
    pub budget_ms: f64,
    /// The parts of `busy_ms`: what is left of it is unmeasured.
    pub door_ms: f64,
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
    /// Requests waiting for a credit, and out on one, by stream.
    pub waiting: [f64; STREAMS],
    pub out: [f64; STREAMS],
    pub terrain_refused: f64,
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
    t => FRAME::t, interval_ms => FRAME::interval_ms, display_ms => FRAME::display_ms, settled => FRAME::settled,
    turns => FRAME::turns, busy_ms => FRAME::busy_ms, budget_ms => FRAME::budget_ms, door_ms => FRAME::door_ms, effects_ms => FRAME::effects_ms, tick_ms => FRAME::tick_ms,
    phases => FRAME::phases, stage_ms => FRAME::stage_ms, tasks_ms => FRAME::tasks_ms,
    worst_effect_ms => FRAME::worst_effect_ms, worst_effect_name => FRAME::worst_effect_name,
    kind => FRAME::kind_name, wake => FRAME::wake_name, wake_in_ms => FRAME::wake_in_ms,
    gpu.ms => FRAME::gpu_ms, gpu.spans => FRAME::gpu_spans,
    gpu.census => FRAME::census, gpu.px => FRAME::px,
    device.buffers => FRAME::buffers, device.buffer_mb => FRAME::buffer_mb,
    device.depth_mb => FRAME::tex_depth_mb, device.layers_mb => FRAME::tex_layers_mb, device.objects => FRAME::gpu_objects,
    tiles.by_zoom => FRAME::zoom_tiles, tiles.place => FRAME::tiles_place,
    heap.peak_bytes => FRAME::heap_peak_bytes, heap.lent_bytes => FRAME::heap_lent_bytes, heap.run_bytes => FRAME::heap_run_bytes,
    heap.held_bytes => FRAME::heap_held_bytes, heap.range_bytes => FRAME::heap_range_bytes,
    heap.largest_free_bytes => FRAME::heap_largest_free_bytes, heap.refusals => FRAME::heap_refusals, heap.releases => FRAME::heap_releases,
    rings.cq_peak => FRAME::ring_cq_peak, rings.held_peak => FRAME::ring_held_peak,
    waiting => FRAME::waiting, out => FRAME::out,
    terrain_refused => FRAME::cap_terrain_refused,
);

words!(WorstFrame, WORST::WORDS, sample => WORST::sample, has_prev => WORST::has_prev, prev => WORST::prev);

pub const GPU_SPANS: usize = GPU_SPAN_NAMES.len();
pub const STREAMS: usize = crate::io::spec::STREAM_NAMES.len();
pub const ZOOMS: usize = LIMITS::ZOOMS as usize;

const _: () = assert!(
    pxl_spec::space::layer::lattice::GROUND.tile_zoom(OBJECT_FINEST) < ZOOMS as i32,
    "a zoom slot for every cell drawn, the finest object tier to the pyramid's top"
);

const MISSED_INTERVALS: f64 = 1.5;

/// A frame that dropped a vsync while moving. Still frames take a larger
/// budget on purpose, and a repeated image is no visible stutter.
pub fn missed(f: &FrameSample) -> bool {
    f.settled == 0.0 && f.interval_ms > f.display_ms * MISSED_INTERVALS
}

/// Past even the hard ceiling: one step too big for the forward-progress waiver.
pub fn overran(f: &FrameSample) -> bool {
    f.tick_ms > f.budget_ms + crate::sched::tick::HARD_SLACK_MS
}

/// What the interval's turns left of it, and what they ran past it: one of
/// the two 0, read off the interval and the turns rather than kept beside them.
pub fn off_turn_ms(f: &FrameSample) -> f64 {
    (f.interval_ms - f.busy_ms).max(0.0)
}

pub fn over_ms(f: &FrameSample) -> f64 {
    (f.busy_ms - f.interval_ms).max(0.0)
}

/// The interval's turns ran past it: a long frame, and `over_ms` by how much.
pub fn long(f: &FrameSample) -> bool {
    over_ms(f) > 0.0
}
