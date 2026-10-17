// pxl-kernel/profiler/report.rs

//! A frame turn's record: what the turn measured of itself, what the renderer
//! made of its frame, and the layers' residency, taken once its frame is
//! drawn, so every figure in it is that one turn's.

use pxl_render::gpu::span_spec::FrameKind;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::sched::tick::TickReport;

use crate::io::request::stream_counts;
use crate::layer::Base;
use crate::profiler::frame::{DeviceSample, FrameSample, GpuSample, HeapSample, TileSample};
use crate::profiler::record::{profiler_record_frame, profiler_recording, profiler_take_rings};

/// What the renderer made of this turn's frame (scene/stage.rs `stage_render`).
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
    let h = crate::io::host::heap_stat();
    HeapSample {
        peak_bytes: h.used as f64,
        lent_bytes: h.live as f64,
        run_bytes: h.run as f64,
        held_bytes: h.slot as f64,
        range_bytes: h.range as f64,
        largest_free_bytes: h.largest_free as f64,
        refusals: h.refusals as f64,
        releases: h.releases as f64,
    }
}

/// A frame turn's own measures (turn.rs `world_frame`).
pub struct FrameTurn {
    pub tick: TickReport,
    /// The host's gap that ended at this frame, which closes the interval
    /// before it.
    pub interval_ms: f64,
    pub display_ms: f64,
    pub settled: bool,
    pub effects_ms: f64,
    pub stage_ms: f64,
    /// The frame task's whole time, which the turn's end takes out of its tasks'.
    pub task_ms: f64,
}

/// The frame turn's record, once its frame is drawn.
pub fn profiler_frame(t: FrameTurn) {
    // Taken whether or not recording, so a recording starts from now.
    let rings = profiler_take_rings();
    let terrain_refused = TerrainTile::residency().take_refusals() as f64;
    let host = core::mem::take(HOST.get(HostFrame::default));
    if !profiler_recording() {
        return;
    }
    let mut tiles = TileSample::default();
    for (p, _) in TerrainTile::residency().held() {
        tiles.by_zoom[p.zoom as usize] += 1.0;
    }
    tiles.place = PlaceTile::residency().held().count() as f64;
    let (worst_effect_ms, worst_effect) = crate::turn::worst_effect();
    let [(waiting_terrain, out_terrain), (waiting_place, out_place)] = [TerrainTile::STREAM, PlaceTile::STREAM].map(|s| {
        let (waiting, out) = stream_counts(s);
        (f64::from(waiting), f64::from(out))
    });
    let record = FrameSample {
        t: now(),
        display_ms: t.display_ms,
        settled: if t.settled { 1.0 } else { 0.0 },
        budget_ms: t.tick.budget_ms,
        effects_ms: t.effects_ms,
        tick_ms: t.tick.total_ms,
        phases: t.tick.phase_ms,
        stage_ms: t.stage_ms,
        worst_effect_ms,
        worst_effect_name: worst_effect.map_or(-1.0, f64::from),
        kind: f64::from(host.kind as u8),
        gpu: host.gpu,
        device: host.device,
        tiles,
        heap: heap_sample(),
        rings,
        waiting_terrain,
        out_terrain,
        waiting_place,
        out_place,
        terrain_refused,
        // The interval's: its turns' as they end, and its length as the next frame closes it.
        ..FrameSample::default()
    };
    profiler_record_frame(record, t.interval_ms, t.task_ms);
}
