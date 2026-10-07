// pxl-kernel/world/chunk/stage.rs

//! The world's part of the tick: in the visible phase it composes the frame
//! and runs the table's pass; in the report phase it hands the frame's
//! numbers to the profiler.

use crate::layer::Query;
use pxl_spec::space::coord::v3::sub3;
use xpute_core::math::scalar::round;
use xpute_guest::sched::edge::{notified, yield_if_spent};

use crate::io::sched::pending_count;
use crate::layer::Layer;
use crate::mem::MB;
use crate::scene::environment::state::globe;
use crate::sched::edge::{request_world_tick, world_edge};
use crate::sched::profiler::frame::{CapSample, CoverSample, DeviceSample, FrameSample, GpuSample, HeapSample, LodSample, MountSample, StaleSample, TileSample, TurnSample, ViewSample, WorkSample};
use crate::sched::profiler::record::{profiler_record_frame, profiler_recording, profiler_take_rings};
use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::world::chunk::decor::build::DecorHide;
use crate::world::chunk::table::{builds, Census, Frame};
use crate::world::view::frame::pose::{sea_under_eye, view, view_resize};
use crate::{kernel, with_world};
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::poi_tile::PoiTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;

const BOX_TRIANGLES: f64 = 12.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostFrameStat {
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
}

struct CoreTick {
    visible_pass: Option<f64>,
    last_work: (u32, u32),
    host: HostFrameStat,
    draw: bool,
}

static CORE_TICK: Global<CoreTick> = Global::new();

fn core_tick() -> &'static mut CoreTick {
    CORE_TICK.get(|| CoreTick {
        visible_pass: None,
        last_work: (0, 0),
        host: HostFrameStat::default(),
        draw: false,
    })
}

pub fn set_host_frame_stat(gpu: GpuSample, device: DeviceSample) {
    let host = &mut core_tick().host;
    (host.gpu, host.device) = (gpu, device);
}

pub fn set_host_task_ms(task_ms: f64) {
    core_tick().host.task_ms = task_ms;
    // Known only after the turn, when its frame is already recorded.
    if let Some(frame) = crate::sched::profiler::record::profiler_open_frame() {
        frame.task_ms = task_ms;
    }
}

/// Written onto the open frame like `set_host_task_ms`, so the parts of
/// `task_ms` add up and unmeasured time shows as the difference.
pub fn set_turn_timing(effects_ms: f64, stage_ms: f64) {
    let Some(frame) = crate::sched::profiler::record::profiler_open_frame() else { return };
    let (step_ms, step_name) = crate::sched::turn::worst_effect();
    frame.turn = TurnSample {
        effects_ms,
        stage_ms,
        step_ms,
        step_name: step_name.map_or(-1.0, f64::from),
    };
}

pub fn take_draw() -> bool {
    core::mem::take(&mut core_tick().draw)
}

fn work_sample(c: &mut CoreTick, census: &Census) -> WorkSample {
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
    let h = crate::door::host::heap_stat();
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

pub fn core_tick_visible_step(ctx: &mut TickContext) {
    let c = core_tick();
    c.visible_pass = None;
    view_resize(ctx.state.width, ctx.state.height);
    let v = view();

    let anchor = sea_under_eye();
    let e = v.pose.eye;
    let frame = Frame {
        anchor,
        eye: e,
        frustum: v.frustum.about(anchor),
        camera: sub3(&e, &anchor),
        delta_s: ctx.delta,
        remaining_ms: ctx.budget.remaining(),
        hard_slack_ms: HARD_SLACK_MS,
        globe: Some(globe()),
    };

    let t0 = now();
    let draw = with_world(|_, sched, scene| sched.tick(frame, &now, scene)).unwrap_or(false);
    // Decor lists after the chunks have listed theirs.
    {
        let k = kernel();
        let edits = crate::scene::marker::hover_cursor::render_edits();
        let hide = edits.hide.filter(|_| edits.hide_in.1).map(DecorHide::Placed).or(edits.scattered.map(DecorHide::Scattered));
        crate::world::chunk::decor::cell::step(&crate::world::chunk::decor::kind::KINDS, &k.objects, &k.sched, &mut k.scene.draw, hide, frame.eye, &ctx.budget);
    }
    crate::world::chunk::place::step(ctx);
    let scene = &mut kernel().scene;
    scene.billboards.draw(&mut scene.draw, &scene.models, frame.eye);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
    c.visible_pass = Some(now() - t0);
}

/// Advances the builds on what the quota leaves after the tick, at least one
/// step a turn, and waits for the next tick once a step does nothing.
pub async fn build_task() {
    loop {
        notified(builds).await;
        loop {
            yield_if_spent(world_edge).await;
            let budget = world_edge().remaining();
            let worked = with_world(|world, sched, _| sched.build_step(world, budget)).unwrap_or(false);
            if !worked {
                break;
            }
            request_world_tick();
        }
    }
}

/// Runs after every other phase, so `sched_ms` covers them all.
pub fn core_tick_report_step(ctx: &mut TickContext) {
    let c = core_tick();
    // No chunks this frame: a search jump empties the stage for a moment.
    let Some(chunks_ms) = c.visible_pass else { return };
    let frame_t0 = ctx.budget.t0;
    let census = kernel().sched.census(profiler_recording(), frame_t0);
    // Taken whether or not recording, so a recording starts from now.
    let counts = kernel().sched.take_counts();
    let rings = profiler_take_rings();
    let work = work_sample(c, &census);
    let record_refused = Terrain::residency().take_refusals() as f64;
    let [rays, visits] = crate::world::chunk::domain::take_work();
    if !profiler_recording() {
        return;
    }
    let host = c.host.clone();
    let mut tiles = TileSample::default();
    for (p, _) in Terrain::residency().held() {
        tiles.by_zoom[p.zoom as usize] += 1.0;
    }
    tiles.total = tiles.by_zoom.iter().sum();
    tiles.poi = PoiTile::residency().held().count() as f64;
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
        q_terrain: pending_count(Terrain::CATEGORY) as f64,
        q_poi: pending_count(PoiTile::CATEGORY) as f64,
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
            let domain = crate::world::chunk::domain::current();
            // NaN: step 0 is past the arena
            let rule = domain.map_or(0.0, |d| d.counted(0).map_or(f64::NAN, f64::from));
            // The goal's pieces a resident tile draws.
            let residency = Terrain::residency();
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
