// pxl-kernel/profiler/report.rs

//! A frame turn's record: what the turn measured of itself, what the renderer
//! made of its frame, and the layers' tile caches, taken once its frame is
//! drawn, so every figure in it is that one turn's.

use pxl_render::gpu::span_spec::FrameKind;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::sched::tick::TickReport;

use crate::domain::layer::Base;
use crate::door;
use crate::profiler::frame::{
    DeviceSample, FrameSample, GpuSample, HeapSample, RingSample, TileSample,
};
use crate::profiler::record::{profiler_record_frame, profiler_record_gpu, profiler_recording};
use crate::sys::boot;
use crate::sys::io::body;
use crate::sys::io::demand::Stream;
use crate::sys::io::request::stream_counts;
use crate::turn;

/// What the renderer made of this turn's frame (domain/scene/stage.rs `stage_render`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct HostFrame {
    kind: FrameKind,
    gpu: GpuSample,
    device: DeviceSample,
}

impl Default for HostFrame {
    fn default() -> Self {
        HostFrame {
            kind: FrameKind::none,
            gpu: GpuSample::default(),
            device: DeviceSample::default(),
        }
    }
}

static HOST: Global<HostFrame> = Global::new();

/// The renderer's answer for this turn's frame, its figures zero where it
/// drew none.
pub fn set_host_frame(kind: FrameKind, gpu: GpuSample, device: DeviceSample) {
    HOST.set(HostFrame { kind, gpu, device });
}

/// The heap as its allocator states it, in its own bytes.
fn heap_sample() -> HeapSample {
    let h = boot::heap_stat();
    HeapSample {
        used_bytes: h.used as f64,
        live_bytes: h.live as f64,
        run_bytes: h.run as f64,
        slot_bytes: h.slot as f64,
        range_bytes: h.range as f64,
        largest_free_bytes: h.largest_free as f64,
        refusals: h.refusals as f64,
    }
}

/// The rings as the door states them since the frame before, which taking
/// them starts again.
fn ring_sample() -> RingSample {
    let s = door::take_stat();
    RingSample {
        cq_peak: f64::from(s.cq_peak),
        sq_peak: f64::from(s.sq_peak),
        held_peak: f64::from(s.left_peak),
    }
}

/// A frame turn's own measures (turn.rs `world_frame`).
pub struct FrameTurn {
    pub tick: TickReport,
    /// The host's gap that ended at this frame, which closes the interval
    /// before it, and whether the host settled it, as it says with this frame.
    pub interval_ms: f64,
    /// The display's refresh and the interval the host paced this frame at
    /// (turn.rs `HostFrame`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub settled: bool,
    pub effects_ms: f64,
    pub stage_ms: f64,
    /// The frame task's whole time, which the turn's end takes out of its tasks'.
    pub task_ms: f64,
}

/// The frame turn's record, once its frame is drawn.
pub fn profiler_frame(t: FrameTurn) {
    // Taken whether or not recording, so a recording starts from now.
    let rings = ring_sample();
    let body_slots_peak = f64::from(body::take_peak());
    let terrain_refused = TerrainTile::tile_cache().take_refusals() as f64;
    let host = core::mem::take(HOST.get(HostFrame::default));
    if !profiler_recording() {
        return;
    }
    let mut tiles = TileSample::default();
    for (p, _) in TerrainTile::tile_cache().held() {
        tiles.by_zoom[p.zoom as usize] += 1.0;
    }
    tiles.place = PlaceTile::tile_cache().held().count() as f64;
    let counts = Stream::ALL.map(stream_counts);
    let record = FrameSample {
        t: now(),
        refresh_ms: t.refresh_ms,
        paced_ms: t.paced_ms,
        budget_ms: t.tick.budget_ms,
        effects_ms: t.effects_ms,
        tick_ms: t.tick.total_ms,
        phases: t.tick.phase_ms,
        stage_ms: t.stage_ms,
        effects: turn::effect_times(),
        kind: f64::from(host.kind as u8),
        gpu: host.gpu,
        device: host.device,
        tiles,
        heap: heap_sample(),
        rings,
        body_slots_peak,
        waiting: counts.map(|c| f64::from(c.0)),
        out: counts.map(|c| f64::from(c.1)),
        terrain_refused,
        // The interval's: its turns' as they end, and its length as the next frame closes it.
        ..FrameSample::default()
    };
    profiler_record_frame(record, t.interval_ms, t.settled, t.task_ms);
}
