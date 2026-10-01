// pxl-kernel/world/chunk/stage.rs

//! The world's turn of the tick. In the visible phase it composes the frame
//! the scheduler reads — the view, the frustum, the budget — and runs the
//! scheduler's pass once; the view's renderer (pxl-render's
//! gpu/renderer.rs) then draws the scheduler's lists. In the report phase it
//! hands the frame's numbers to the profiler.

use pxl_spec::space::pvf3::ivec;
use xpute_core::math::scalar::round;
use xpute_runtime::sched::edge::{notified, yield_if_spent};

use crate::global::Global;
use crate::host::now;
use crate::io::sched::pending_count;
use crate::kernel::{kernel, with_world};
use crate::layer::decor::build::DecorHide;
use crate::layer::environment::state::globe;
use crate::sched::profiler::record::{
    profiler_record_frame, profiler_recording, profiler_take_rings, CapSample, CoverSample, DeviceSample, FrameSample, GpuSample, HeapSample, LodSample, MountSample, PageSample, StaleSample,
    TurnSample, WorkSample, MB,
};
use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::world::change::{request_world_tick, world_edge};
use crate::world::chunk::table::{builds, Census, Frame};
use crate::world::frame::view::{globe_anchor_local, view, view_resize};
use crate::world::pyramid::Layer;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::poi_page::PoiPage;

/// A box drawn is a cube, twelve triangles, whatever its faces show (the
/// renderer's census counts them the same way).
const BOX_TRIANGLES: f64 = 12.0;

/// What the turn measured of the frame outside the tick: the renderer's
/// numbers and the turn's own time. The terrain's triangles by level are the
/// kernel's, from what it listed, and left for the report step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostFrameStat {
    pub task_ms: f64,
    pub gpu: GpuSample,
    pub device: DeviceSample,
}

struct CoreTick {
    /// How long the visible phase took, handed to the report phase.
    visible_pass: Option<f64>,
    last_work: (u32, u32),
    host: HostFrameStat,
    /// Whether the scheduler's pass left something new to draw.
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

/// The shell's numbers for the frame just drawn.
pub fn set_host_frame_stat(gpu: GpuSample, device: DeviceSample) {
    let host = &mut core_tick().host;
    (host.gpu, host.device) = (gpu, device);
}

/// The whole turn just run: the strobe in, the tick, the draw and submit.
pub fn set_host_task_ms(task_ms: f64) {
    core_tick().host.task_ms = task_ms;
    // The turn's own task time is only known once the turn is over, by which
    // point its frame is recorded; without this the number would be read as
    // the next frame's and every frame would carry the one before it.
    if let Some(frame) = crate::sched::profiler::record::profiler_last_frame() {
        frame.task_ms = task_ms;
    }
}

/// The turn's parts, once the turn is over: what the effects took and what
/// the stage took, written onto the frame already recorded the way
/// `set_host_task_ms` is. With the tick's own numbers beside them,
/// `task_ms` is accounted for — and what none of the three explains is the
/// difference, where a region nobody measured would have to show itself.
pub fn set_turn_timing(effects_ms: f64, stage_ms: f64) {
    let Some(frame) = crate::sched::profiler::record::profiler_last_frame() else { return };
    let (step_ms, step_name) = crate::sched::turn::worst_effect();
    frame.turn = TurnSample {
        effects_ms,
        stage_ms,
        step_ms,
        step_name: step_name.map_or(-1.0, f64::from),
    };
}

/// Whether the last pass left something new to draw; reading clears it.
pub fn take_draw() -> bool {
    core::mem::take(&mut core_tick().draw)
}

/// The scheduler's builds since the last sample: scans (the direct path counted
/// with them) and bakes.
fn work_sample(c: &mut CoreTick, census: &Census) -> WorkSample {
    let st = kernel().sched.builder.stat;
    let scans = st.scans + st.direct;
    let sample = WorkSample {
        scans: scans.wrapping_sub(c.last_work.0) as f64,
        bakes: st.bakes.wrapping_sub(c.last_work.1) as f64,
        fine_peak: census.fine_peak as f64,
        coarse_peak: census.coarse_peak as f64,
    };
    c.last_work = (scans, st.bakes);
    sample
}

/// The heap as the allocator's seam reads it (host.rs), in MiB. `used` is a
/// high-water mark (`max_ptr - base_ptr`) while `live` is what is out on loan,
/// so the two are not each other's complement; a request is served or it is
/// refused, and `refusals` counts the second. `run` and `slot` are not
/// rounded, because what the carving costs is their difference and it is
/// smaller than what rounding would hide.
fn heap_sample() -> HeapSample {
    let h = crate::host::heap_stat();
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

/// The visible phase's step: the frame, and the scheduler's pass over every
/// chunk.
pub fn core_tick_visible_step(ctx: &mut TickContext) {
    let c = core_tick();
    c.visible_pass = None;
    view_resize(ctx.state.width, ctx.state.height);
    let v = view();

    let origin = [ivec::to_real(ivec::X(v.origin)), ivec::to_real(ivec::Y(v.origin)), ivec::to_real(ivec::Z(v.origin))];

    let anchor = globe_anchor_local();
    let mut planes = v.planes;
    for p in planes.as_chunks_mut::<4>().0 {
        p[3] += p[0] * anchor[0] + p[1] * anchor[1] + p[2] * anchor[2];
    }
    let e = v.pose.eye;
    let frame = Frame {
        anchor,
        origin,
        eye: v.eye_at(),
        planes,
        camera: [e[0] - anchor[0], e[1] - anchor[1], e[2] - anchor[2]],
        delta_s: ctx.delta,
        remaining_ms: ctx.budget.remaining(),
        hard_slack_ms: HARD_SLACK_MS,
        center_z: ivec::to_real(ivec::Z(v.origin)) + anchor[2],
        globe: Some(globe()),
    };

    let t0 = now();
    let draw = with_world(|world, sched| sched.tick(world, frame, &now)).unwrap_or(false);
    // Decor, its own layer: its frontier, its blocks and how each is drawn,
    // after the chunks have listed theirs (layer/decor/cell.rs).
    {
        let k = kernel();
        let edits = crate::layer::object::store::render_edits();
        let hide = edits
            .hide
            .filter(|_| edits.hide_in.1)
            .map(DecorHide::Placed)
            .or(crate::layer::decor::cell::scattered_hide().map(DecorHide::Scattered));
        crate::layer::decor::cell::step(crate::layer::decor::build::kinds(), &k.objects, &mut k.sched.draw, hide, frame.eye);
    }
    crate::layer::poi::placement::step(ctx);
    let sched = &mut kernel().sched;
    sched.billboards.draw(&mut sched.draw, &sched.models, frame.eye);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
    c.visible_pass = Some(now() - t0);
}

/// The build task: the chunks each tick orders, advanced on what the turn's
/// quota leaves after the tick, one step a turn at least. It waits while no
/// tick has given it anything, and a turn whose builds did nothing — all
/// built, blocked, or a block refused — waits for the next tick's order.
pub async fn build_task() {
    loop {
        notified(builds).await;
        loop {
            yield_if_spent(world_edge).await;
            let budget = world_edge().remaining();
            let worked = with_world(|world, sched| sched.build_step(world, budget)).unwrap_or(false);
            if !worked {
                break;
            }
            request_world_tick();
        }
    }
}

/// The report phase's step: the frame's numbers, after every other phase has
/// run, so `sched_ms` covers every phase.
pub fn core_tick_report_step(ctx: &mut TickContext) {
    let c = core_tick();
    // No chunks this frame — a search jump empties the stage for a moment.
    let Some(chunks_ms) = c.visible_pass else { return };
    let frame_t0 = ctx.budget.t0;
    let census = kernel().sched.census;
    // What is counted since the last sample is taken whether or not a
    // recording keeps it, so a recording starts from now.
    let counts = kernel().sched.take_counts();
    let rings = profiler_take_rings();
    let work = work_sample(c, &census);
    let record_refused = Terrain::pyramid().take_refusals() as f64;
    // The rest is measured only for a recording to keep.
    if !profiler_recording() {
        return;
    }
    let host = c.host.clone();
    let mut pages = PageSample::default();
    for (p, _) in Terrain::pyramid().paging().held() {
        pages.by_level[p.level as usize] += 1.0;
    }
    pages.total = pages.by_level.iter().sum();
    pages.poi = PoiPage::pyramid().paging().held().count() as f64;
    let mut gpu = host.gpu;
    for (tris, boxes) in gpu.level_tris.iter_mut().zip(census.level_boxes) {
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
        pages,
        heap: heap_sample(),
        rings,
        work,
        q_terrain: pending_count(Terrain::CATEGORY) as f64,
        q_poi: pending_count(PoiPage::CATEGORY) as f64,
        stale: StaleSample {
            hidden: census.ready_waiting as f64,
            slot_waits: census.slot_waits as f64,
            wait_levels: census.wait_levels as f64,
            chained: census.chained as f64,
            uncovered: census.refused as f64,
            unready: census.waiting as f64,
        },
        turn: TurnSample::default(),
        cover: {
            let (cover, _) = crate::world::pyramid::cover::cover::<Terrain>();
            // The target's cut is made for this where the quality in force
            // is coarser, which only a view past its arena pays for.
            let rule = crate::world::pyramid::domain::cut_at(0).map_or(0.0, |(_, cut)| cut.pieces().len() as f64);
            let domain = crate::world::pyramid::domain::current();
            CoverSample {
                rule,
                drawn: cover.drawn.len() as f64,
                gaps: domain.map_or(0.0, |d| d.unanswered() as f64),
            }
        },
        cap: CapSample {
            commit_left: census.commit_left as f64,
            mount_left: census.refused as f64,
            mount_full: (census.refused > 0) as u32 as f64,
            record_refused,
        },
    });
}
