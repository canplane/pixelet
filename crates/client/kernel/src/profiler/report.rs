// pxl-kernel/profiler/report.rs

//! A turn's frame record: what the host measured, the chunks' census and the
//! layers' residency, taken after every other phase.

use pxl_spec::bake::format::TerrainTile;
use pxl_spec::bake::place_tile::PlaceTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_kit::math::scalar::round;

use crate::chunk::table::Census;
use crate::io::request::pending_count;
use crate::kernel;
use crate::layer::{Base, Query};
use crate::mem::MB;
use crate::profiler::frame::{CapSample, CoverSample, DeviceSample, FrameSample, GpuSample, HeapSample, LodSample, MountSample, StaleSample, TileSample, TurnSample, ViewSample, WorkSample};
use crate::profiler::record::{profiler_record_frame, profiler_recording, profiler_take_rings};
use crate::scene::stage::visible_pass;
use crate::sched::tick::TickContext;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostFrameStat {
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
}

const BOX_TRIANGLES: f64 = 12.0;

struct Report {
    last_work: (u32, u32),
    host: HostFrameStat,
}

static REPORT: Global<Report> = Global::new();

fn report() -> &'static mut Report {
    REPORT.get(|| Report {
        last_work: (0, 0),
        host: HostFrameStat::default(),
    })
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

fn work_sample(c: &mut Report, census: &Census) -> WorkSample {
    let st = kernel().sched.builder.stat;
    let runs = st.runs;
    let sample = WorkSample {
        runs: runs.wrapping_sub(c.last_work.0) as f64,
        bakes: st.bakes.wrapping_sub(c.last_work.1) as f64,
        fine_peak: census.fine_peak as f64,
        coarse_peak: census.coarse_peak as f64,
    };
    c.last_work = (runs, st.bakes);
    sample
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
    // No chunks this frame: a search jump empties the stage for a moment.
    let Some(chunks_ms) = visible_pass() else { return };
    let frame_t0 = ctx.budget.t0;
    let census = kernel().sched.census(profiler_recording(), frame_t0);
    // Taken whether or not recording, so a recording starts from now.
    let counts = kernel().sched.take_counts();
    let rings = profiler_take_rings();
    let work = work_sample(c, &census);
    let record_refused = TerrainTile::residency().take_refusals() as f64;
    let [rays, visits] = crate::chunk::domain::take_work();
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
    let mut gpu = host.gpu;
    for (tris, boxes) in gpu.zoom_tris.iter_mut().zip(census.zoom_boxes) {
        *tris = boxes as f64 * BOX_TRIANGLES;
    }
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
        gpu,
        device: host.device,
        mounts: MountSample {
            added: counts.added as f64,
            removed: counts.removed as f64,
            layouts: counts.layouts as f64,
        },
        lod: LodSample {
            chunks: census.chunks as f64,
            pending: census.pending as f64,
            in_frustum: census.in_frustum as f64,
            settling: census.settling as f64,
            offscreen_pending: census.offscreen_pending as f64,
            oldest_release_ms: census.oldest_offscreen_ms,
            oldest_visible_ms: census.oldest_visible_ms,
        },
        tiles,
        heap: heap_sample(),
        rings,
        work,
        q_terrain: pending_count(TerrainTile::CATEGORY) as f64,
        q_place: pending_count(PlaceTile::CATEGORY) as f64,
        stale: StaleSample {
            hidden: census.ready_waiting as f64,
            slot_waits: census.slot_waits as f64,
            wait_zooms: census.wait_zooms as f64,
            chained: census.chained as f64,
            uncovered: census.refused as f64,
            unready: census.waiting as f64,
        },
        turn: TurnSample::default(),
        cover: {
            let domain = crate::chunk::domain::current();
            // NaN: step 0 is past the arena
            let rule = domain.map_or(0.0, |d| d.counted(0).map_or(f64::NAN, f64::from));
            // The goal's pieces a resident tile draws.
            let residency = TerrainTile::residency();
            let drawn = domain.map_or(0, |d| d.pieces().iter().filter(|c| residency.read(c.node(), c.cell_zoom()).value().is_some()).count());
            CoverSample {
                rule,
                drawn: drawn as f64,
                gaps: domain.map_or(0.0, |d| d.unanswered() as f64),
            }
        },
        view: ViewSample {
            rays: rays as f64,
            visits: visits as f64,
        },
        cap: CapSample {
            commit_left: census.commit_left as f64,
            mount_left: census.refused as f64,
            mount_full: (census.refused > 0) as u32 as f64,
            record_refused,
        },
    });
}
