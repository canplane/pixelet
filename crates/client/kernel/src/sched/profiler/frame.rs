// pxl-kernel/sched/profiler/frame.rs

//! A frame's record, the worst ones kept, and the peaks since recording began.

use core::mem::{offset_of, size_of};

use pxl_spec::bake::constant::OBJECT_FINEST;
use xpute_runtime::sched::tick::PHASES;

use crate::sched::profiler::spec::{CENSUS_TAGS, FRAME, GPU_SPAN_NAMES, LIMITS, PHASE_NAMES, SUMMARY, WORST};
use crate::sched::profiler::word::{words, Words};

pub const CENSUS_LEN: usize = CENSUS_TAGS.len();

/// (draws, triangles) per pipeline, in CENSUS_TAGS' order.
pub type DrawCensus = [[f64; 2]; CENSUS_LEN];

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuSample {
    /// Lags a frame or two; 0 before the renderer exists or without GPU timing.
    pub ms: f64,
    pub spans: [f64; GPU_SPANS],
    pub draw_calls: f64,
    pub triangles: f64,
    pub census: DrawCensus,
    /// Slot z: triangles of cells at zoom z.
    pub zoom_tris: [f64; ZOOMS],
    pub px: f64,
    /// 1 where the frame did not fit the GPU stream and was not drawn.
    pub skipped: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DeviceSample {
    pub buffers: f64,
    pub buffer_mb: f64,
    pub arena_mb: f64,
    pub depth_mb: f64,
    pub shadow_mb: f64,
    pub layers_mb: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LodSample {
    pub chunks: f64,
    pub in_frustum: f64,
    pub pending: f64,
    pub settling: f64,
    /// Pending chunks outside the frustum. They sort last, so starvation shows here first.
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountSample {
    pub added: f64,
    pub removed: f64,
    pub layouts: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TileSample {
    pub total: f64,
    pub by_zoom: [f64; ZOOMS],
    pub poi: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeapSample {
    pub used_mb: f64,
    /// Handed out by the page allocator, carved runs counted whole.
    pub live_mb: f64,
    /// Of `live_mb`, what was carved into slots, and what is out in slots;
    /// the difference is the carving's overhead.
    pub run_mb: f64,
    pub slot_mb: f64,
    pub range_mb: f64,
    pub refusals: f64,
    pub largest_free_mb: f64,
}

/// The completion ring's peak, and the most submissions a turn held back for
/// lack of room (each answered a frame late).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RingSample {
    pub cq_peak: f64,
    pub held_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorkSample {
    pub runs: f64,
    pub bakes: f64,
    /// Largest mesh handed over since the last frame, in boxes, per slot kind.
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSample {
    pub t: f64,
    /// The host's rAF interval, not the world's step: they differ past
    /// MAX_STEP_S, which is where the frames worth reading are.
    pub dt_ms: f64,
    pub frame_ms: f64,
    /// 1 when the tick took the large settled budget, so a long dt was chosen.
    pub settled: f64,
    /// From the turn's rising edge to the end of the tick: what `budget_ms` is a
    /// quota on. It exceeds the sum of `phases` by the effects and wait before the
    /// frame task.
    pub sched_ms: f64,
    pub budget_ms: f64,
    pub phases: [f64; PHASES.len()],
    pub chunks_ms: f64,
    /// The kernel's whole turn; `dt_ms - task_ms` is everything outside it.
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
    pub lod: LodSample,
    pub mounts: MountSample,
    pub tiles: TileSample,
    pub heap: HeapSample,
    pub rings: RingSample,
    pub work: WorkSample,
    // Queued plus in-flight requests, per category.
    pub q_terrain: f64,
    pub q_poi: f64,
    pub stale: StaleSample,
    pub cap: CapSample,
    pub turn: TurnSample,
    pub cover: CoverSample,
    pub view: ViewSample,
}

/// The goal's rays cast and nodes visited this turn.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewSample {
    pub rays: f64,
    pub visits: f64,
}

/// Chunks the goal would be at target quality, pieces at the effective one,
/// and columns at the top with nothing resident yet (`unanswered`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoverSample {
    pub rule: f64,
    pub drawn: f64,
    pub gaps: f64,
}

/// Lane names follow profiler.json.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StaleSample {
    pub hidden: f64,
    pub uncovered: f64,
    pub unready: f64,
    pub slot_waits: f64,
    pub wait_zooms: f64,
    pub chained: f64,
}

/// The turn in parts that add up to `task_ms`, so unmeasured time shows as
/// the difference. Effects went untimed until one took 70 ms of a turn whose
/// tick was 1.5 ms (2026-09-23).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnSample {
    pub effects_ms: f64,
    pub stage_ms: f64,
    pub step_ms: f64,
    pub step_name: f64,
}

/// What the frame's ceilings left undone: whether one is holding the screen back.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CapSample {
    pub commit_left: f64,
    pub mount_left: f64,
    pub mount_full: f64,
    pub record_refused: f64,
}

/// Kept with the frame before it, since geometry, textures and heap only mean anything as deltas.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorstFrame {
    pub sample: FrameSample,
    pub has_prev: f64,
    pub prev: FrameSample,
}

/// Worst values since recording started, kept past the ring's minute.
/// Written through `words!`: a hand-ordered copy once shifted every lane
/// from the fifth on by one (2026-09-23).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FramePeak {
    pub present: f64,
    pub dt_ms: f64,
    pub sched_ms: f64,
    pub chunks_ms: f64,
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub q_terrain: f64,
    pub q_poi: f64,
    pub chunks: f64,
    pub in_frustum: f64,
    pub pending: f64,
    pub settling: f64,
    pub offscreen_pending: f64,
    pub oldest_release_ms: f64,
    pub oldest_visible_ms: f64,
    pub fine_peak: f64,
    pub coarse_peak: f64,
}

words!(FrameSample, FRAME::WORDS,
    t => FRAME::t, dt_ms => FRAME::dt_ms, frame_ms => FRAME::frame_ms, settled => FRAME::settled,
    sched_ms => FRAME::sched_ms, budget_ms => FRAME::budget_ms, phases => FRAME::phases, chunks_ms => FRAME::chunks_ms, task_ms => FRAME::task_ms,
    gpu.ms => FRAME::gpu_ms, gpu.spans => FRAME::gpu_spans, gpu.draw_calls => FRAME::draw_calls, gpu.triangles => FRAME::triangles,
    gpu.census => FRAME::census, gpu.zoom_tris => FRAME::zoom_tris, gpu.px => FRAME::px, gpu.skipped => FRAME::gpu_skipped,
    device.buffers => FRAME::buffers, device.buffer_mb => FRAME::buffer_mb, device.arena_mb => FRAME::arena_mb,
    device.depth_mb => FRAME::tex_depth_mb, device.shadow_mb => FRAME::tex_shadow_mb, device.layers_mb => FRAME::tex_layers_mb,
    lod.chunks => FRAME::lod_chunks, lod.in_frustum => FRAME::lod_in_frustum, lod.pending => FRAME::lod_pending, lod.settling => FRAME::lod_settling,
    lod.offscreen_pending => FRAME::lod_offscreen_pending,
    lod.oldest_release_ms => FRAME::lod_oldest_release_ms, lod.oldest_visible_ms => FRAME::lod_oldest_visible_ms,
    mounts.added => FRAME::mounts_added, mounts.removed => FRAME::mounts_removed, mounts.layouts => FRAME::mounts_layouts,
    tiles.total => FRAME::tiles_total, tiles.by_zoom => FRAME::zoom_tiles, tiles.poi => FRAME::tiles_poi,
    heap.used_mb => FRAME::heap_used_mb, heap.live_mb => FRAME::heap_live_mb, heap.run_mb => FRAME::heap_run_mb,
    heap.slot_mb => FRAME::heap_slot_mb, heap.range_mb => FRAME::heap_range_mb,
    heap.refusals => FRAME::heap_refusals, heap.largest_free_mb => FRAME::heap_largest_free_mb,
    rings.cq_peak => FRAME::ring_cq_peak, rings.held_peak => FRAME::ring_held_peak,
    work.runs => FRAME::work_runs, work.bakes => FRAME::work_bakes, work.fine_peak => FRAME::work_fine_peak, work.coarse_peak => FRAME::work_coarse_peak, q_terrain => FRAME::q_terrain, q_poi => FRAME::q_poi,
    stale.hidden => FRAME::stale_hidden, stale.uncovered => FRAME::stale_uncovered, stale.unready => FRAME::stale_unready,
    stale.slot_waits => FRAME::slot_waits, stale.wait_zooms => FRAME::slot_wait_zooms,
    stale.chained => FRAME::slot_chained,
    cap.commit_left => FRAME::cap_commit_left, cap.mount_left => FRAME::cap_mount_left,
    cap.mount_full => FRAME::cap_mount_full, cap.record_refused => FRAME::cap_record_refused,
    turn.effects_ms => FRAME::turn_effects_ms, turn.stage_ms => FRAME::turn_stage_ms,
    turn.step_ms => FRAME::turn_step_ms, turn.step_name => FRAME::turn_step_name,
    cover.rule => FRAME::cover_rule, cover.drawn => FRAME::cover_drawn, cover.gaps => FRAME::cover_gaps,
    view.rays => FRAME::view_rays, view.visits => FRAME::view_visits,
);

words!(WorstFrame, WORST::WORDS, sample => WORST::sample, has_prev => WORST::has_prev, prev => WORST::prev);

words!(FramePeak, SUMMARY::peak::WORDS,
    present => SUMMARY::peak::present, dt_ms => SUMMARY::peak::dt_ms, sched_ms => SUMMARY::peak::sched_ms,
    chunks_ms => SUMMARY::peak::chunks_ms, gpu_ms => SUMMARY::peak::gpu_ms, draw_calls => SUMMARY::peak::draw_calls,
    triangles => SUMMARY::peak::triangles, q_terrain => SUMMARY::peak::q_terrain, q_poi => SUMMARY::peak::q_poi,
    chunks => SUMMARY::peak::chunks, in_frustum => SUMMARY::peak::in_frustum, pending => SUMMARY::peak::pending,
    settling => SUMMARY::peak::settling,
    offscreen_pending => SUMMARY::peak::offscreen_pending, oldest_release_ms => SUMMARY::peak::oldest_release_ms,
    oldest_visible_ms => SUMMARY::peak::oldest_visible_ms, fine_peak => SUMMARY::peak::fine_peak,
    coarse_peak => SUMMARY::peak::coarse_peak,
);

const _: () = assert!(PHASE_NAMES.len() == PHASES.len(), "profiler.json names the tick's phases");

pub const GPU_SPANS: usize = GPU_SPAN_NAMES.len();
pub const ZOOMS: usize = LIMITS::ZOOMS as usize;

const _: () = assert!(GPU_SPANS == pxl_render::gpu::renderer::GPU_SPANS.len(), "profiler.json names the renderer's spans");

const _: () = assert!(
    pxl_spec::space::layer::lattice::GROUND.tile_zoom(OBJECT_FINEST) < ZOOMS as i32,
    "a zoom slot for every cell drawn, the finest object tier to the pyramid's top"
);
