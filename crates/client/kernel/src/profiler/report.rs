// pxl-kernel/profiler/report.rs

//! A turn's frame record: what the host measured and the layers' residency,
//! taken after every other phase.

use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_kit::math::scalar::round;

use crate::io::request::pending_count;
use crate::layer::Base;
use crate::mem::MB;
use crate::profiler::frame::{CapSample, DeviceSample, FrameSample, GpuSample, HeapSample, TileSample, TurnSample};
use crate::profiler::record::{profiler_record_frame, profiler_recording, profiler_take_rings};
use crate::scene::stage::visible_pass;
use crate::sched::tick::TickContext;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostFrameStat {
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
}

struct Report {
    host: HostFrameStat,
}

static REPORT: Global<Report> = Global::new();

fn report() -> &'static mut Report {
    REPORT.get(|| Report { host: HostFrameStat::default() })
}

pub fn set_host_frame_stat(gpu: GpuSample, device: DeviceSample) {
    let host = &mut report().host;
    (host.gpu, host.device) = (gpu, device);
}

pub fn set_host_task_ms(task_ms: f64) {
    report().host.task_ms = task_ms;
    // Known only after the turn, when its frame is already recorded.
    if let Some(frame) = crate::profiler::record::profiler_open_frame() {
        frame.task_ms = task_ms;
    }
}

/// Written onto the open frame like `set_host_task_ms`, so the parts of
/// `task_ms` add up and unmeasured time shows as the difference.
pub fn set_turn_timing(effects_ms: f64, stage_ms: f64) {
    let Some(frame) = crate::profiler::record::profiler_open_frame() else { return };
    let (step_ms, step_name) = crate::turn::worst_effect();
    frame.turn = TurnSample {
        effects_ms,
        stage_ms,
        step_ms,
        step_name: step_name.map_or(-1.0, f64::from),
    };
}

/// `used` is a high-water mark and `live` what is on loan, so they are not
/// complements. `run` and `slot` are unrounded because their difference is
/// smaller than rounding would hide.
fn heap_sample() -> HeapSample {
    let h = crate::io::host::heap_stat();
    HeapSample {
        used_mb: round(h.used as f64 / MB),
        live_mb: round(h.live as f64 / MB),
        run_mb: h.run as f64 / MB,
        slot_mb: h.slot as f64 / MB,
        range_mb: round(h.range as f64 / MB),
        refusals: h.refusals as f64,
        largest_free_mb: h.largest_free as f64 / MB,
    }
}

/// Runs after every other phase, so `sched_ms` covers them all.
pub fn report_step(ctx: &mut TickContext) {
    let c = report();
    // No visible pass this frame: a search jump empties the stage for a moment.
    let Some(chunks_ms) = visible_pass() else { return };
    let frame_t0 = ctx.budget.t0;
    // Taken whether or not recording, so a recording starts from now.
    let rings = profiler_take_rings();
    let record_refused = TerrainTile::residency().take_refusals() as f64;
    if !profiler_recording() {
        return;
    }
    let host = c.host.clone();
    let mut tiles = TileSample::default();
    for (p, _) in TerrainTile::residency().held() {
        tiles.by_zoom[p.zoom as usize] += 1.0;
    }
    tiles.total = tiles.by_zoom.iter().sum();
    tiles.place = PlaceTile::residency().held().count() as f64;
    profiler_record_frame(FrameSample {
        t: now(),
        dt_ms: ctx.state.gap_s * 1000.0,
        frame_ms: ctx.state.frame_ms,
        settled: if ctx.state.interactive { 0.0 } else { 1.0 },
        sched_ms: now() - frame_t0,
        budget_ms: ctx.budget.total_ms,
        phases: ctx.phase_ms,
        chunks_ms,
        task_ms: host.task_ms,
        gpu: host.gpu,
        device: host.device,
        tiles,
        heap: heap_sample(),
        rings,
        q_terrain: pending_count(TerrainTile::STREAM) as f64,
        q_place: pending_count(PlaceTile::STREAM) as f64,
        turn: TurnSample::default(),
        cap: CapSample { record_refused },
    });
}
