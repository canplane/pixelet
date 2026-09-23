// pxl-kernel/world/chunk/stage.rs

//! The world's turn of the tick. In the visible phase it composes the frame the
//! scheduler reads — the view, the frustum, the budget, the range anchor, the
//! settings — runs the scheduler's pass once, and applies what it told to the
//! chunk nodes (node.rs); the view's renderer (pxl-render's gpu/renderer.rs)
//! then draws the scheduler's lists. In the report phase it hands the frame's
//! numbers to the ledger.

use pxl_render::look::lod::set_lod_projection;
use pxl_spec::space::pvf3::{fvec, ivec};
use xpute_core::math::scalar::round;
use xpute_runtime::sched::edge::{notified, yield_if_spent};

use crate::global::Global;
use crate::host::now;
use crate::io::scheduler::fetch_pending_count;
use crate::kernel::{kernel, with_world};
use crate::layer::environment::state::globe;
use crate::layer::poi::fetcher;
use crate::layer::terrain::level::window;
use crate::layer::terrain::store::tile_pages;
use crate::layer::terrain::tile_fetcher;
use crate::sched::ledger::record::{ledger_record_frame, ledger_take_rings, CpuSample, DrawCensus, FrameSample, HeapSample, LodSample, MountSample, TexturesMb, WorkSample};
use crate::sched::priority::lead_point;
use crate::sched::tick::{TickContext, HARD_SLACK_MS};
use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;
use crate::world::change::{request_world_tick, world_edge};
use crate::world::chunk::node::{run_chunk_events, take_node_counts, RETAIN_BUDGET_BYTES};
use crate::world::chunk::range::range_anchor_of;
use crate::world::chunk::table::{builds, Frame};
use crate::world::frame::view::{view, view_resize};

const MB: f64 = 1_048_576.0;

/// What the turn measured of the frame outside the tick: the renderer's
/// numbers and the turn's own time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostFrameStat {
    pub task_ms: f64,
    pub gpu_ms: f64,
    pub draw_calls: f64,
    pub triangles: f64,
    pub buffers: f64,
    pub buffer_bytes: f64,
    pub textures_mb: TexturesMb,
    pub census: DrawCensus,
    pub px: f64,
}

struct CoreTick {
    /// What the visible phase measured, handed to the report phase.
    visible_pass: Option<(u32, u32, f64)>,
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
pub fn set_host_frame_stat(stat: HostFrameStat) {
    let task_ms = core_tick().host.task_ms;
    core_tick().host = HostFrameStat { task_ms, ..stat };
}

/// The whole turn just run: the strobe in, the tick, the draw and submit.
pub fn set_host_task_ms(task_ms: f64) {
    core_tick().host.task_ms = task_ms;
    // The turn's own task time is only known once the turn is over, by which
    // point its frame is recorded; without this the number would be read as
    // the next frame's and every frame would carry the one before it.
    if let Some(frame) = crate::sched::ledger::record::ledger_last_frame() {
        frame.task_ms = task_ms;
    }
}

/// Whether the last pass left something new to draw; reading clears it.
pub fn take_draw() -> bool {
    core::mem::take(&mut core_tick().draw)
}

/// The scheduler's builds since the last sample: scans (the direct path counted
/// with them) and bakes.
fn work_sample(c: &mut CoreTick) -> WorkSample {
    let st = kernel().sched.builder.stat;
    let scans = st.scans + st.direct;
    let sample = WorkSample {
        scans: scans.wrapping_sub(c.last_work.0) as f64,
        bakes: st.bakes.wrapping_sub(c.last_work.1) as f64,
    };
    c.last_work = (scans, st.bakes);
    sample
}

/// The heap's gauges, straight off the allocator's seam (host.rs). `used` is a
/// high-water mark (`max_ptr - base_ptr`) while `live` is what is out on loan,
/// so the two are not each other's complement; a request is served or it is
/// refused, and `refusals` counts the second.
fn heap_sample() -> HeapSample {
    HeapSample {
        used_mb: round(crate::host::heap_used() as f64 / MB),
        live_mb: round(crate::host::heap_live() as f64 / MB),
        range_mb: round(crate::host::heap_bytes() as f64 / MB),
        refusals: crate::host::heap_refusals() as f64,
        largest_free_mb: crate::host::heap_largest_free() as f64 / MB,
    }
}

/// The visible phase's step: the frame, the scheduler's pass over every chunk,
/// and what it told applied to the nodes.
pub fn core_tick_visible_step(ctx: &mut TickContext) {
    let c = core_tick();
    c.visible_pass = None;
    view_resize(ctx.state.width, ctx.state.height);
    // The tier criterion is a screen-space one (crates/pxl/web/render/src/lod.rs), and the
    // other layers that read it read it from here; the scheduler takes the same
    // projection through the frame, and its pressure — the byte budget paid in
    // LOD — comes back through the census.
    let v = view();
    set_lod_projection(v.height, v.fov, kernel().sched.census.pressure);

    // Where the camera is heading — see sched/priority.rs. The scheduler orders
    // the work by it; the tier each chunk picks still comes from the camera as
    // drawn.
    let target = map_store().map_center;
    let origin = v.origin;
    let lead = match &target {
        Some(t) => {
            let local = ivec::sub(t.i, origin);
            lead_point(
                &v.eye,
                ivec::to_real(ivec::X(local)) + fvec::to_real(fvec::X(t.f)),
                ivec::to_real(ivec::Y(local)) + fvec::to_real(fvec::Y(t.f)),
                ivec::to_real(ivec::Z(local)) + fvec::to_real(fvec::Z(t.f)),
                &v.root,
            )
        }
        None => v.eye,
    };

    let settings = settings_store();
    let range_anchor = target.map(|t| range_anchor_of(t.i));
    let frame = Frame {
        root: v.root,
        origin: [ivec::to_real(ivec::X(origin)), ivec::to_real(ivec::Y(origin)), ivec::to_real(ivec::Z(origin))],
        planes: v.planes,
        camera: [v.eye.x, v.eye.y, v.eye.z],
        lead: [lead.x, lead.y, lead.z],
        viewport_h: v.height,
        fov_deg: v.fov,
        delta_s: ctx.delta,
        budget_ms: ctx.budget.remaining(),
        hard_slack_ms: HARD_SLACK_MS,
        pixel_dither: settings.pixel_dither,
        retain_budget_bytes: RETAIN_BUDGET_BYTES(),
        anchor: range_anchor.map(|a| [ivec::to_real(ivec::X(a)), ivec::to_real(ivec::Y(a))]),
        ground_z: target.map_or(0.0, |t| ivec::to_real(ivec::Z(t.i)) + fvec::to_real(fvec::Z(t.f))),
        globe: Some(globe()),
        window: {
            let w = window();
            (w.start, w.end)
        },
        show_decor: settings.show_decor,
    };

    let t0 = now();
    let mut events = Vec::new();
    let draw = with_world(|world, sched| sched.tick(world, frame, &now, &mut |e| events.push(e))).unwrap_or(false);
    run_chunk_events(events);
    if draw {
        c.draw = true;
        ctx.invalidate();
    }
    let census = kernel().sched.census;
    c.visible_pass = Some((census.pending, census.committed, now() - t0));
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
            let mut events = Vec::new();
            let budget = world_edge().remaining();
            let worked = with_world(|world, sched| sched.build_step(world, budget, &mut |e| events.push(e))).unwrap_or(false);
            run_chunk_events(events);
            if !worked {
                break;
            }
            request_world_tick();
        }
    }
}

/// The report phase's step: the frame's numbers, after every other phase has
/// run, so `sched_ms` is the whole tick.
pub fn core_tick_report_step(ctx: &mut TickContext) {
    let c = core_tick();
    // No chunks this frame — a search jump empties the stage for a moment.
    let Some((pending_total, committed, tick_ms)) = c.visible_pass else { return };
    let frame_t0 = ctx.budget.t0;
    let census = kernel().sched.census;
    let node_work = take_node_counts();
    let poi_count: f64 = 0.0;
    let host = c.host.clone();
    ledger_record_frame(FrameSample {
        t: now(),
        dt_ms: ctx.delta * 1000.0,
        frame_ms: ctx.state.frame_ms,
        settled: if ctx.state.interactive { 0.0 } else { 1.0 },
        sched_ms: now() - frame_t0,
        budget_ms: ctx.budget.total_ms,
        phases: ctx.phase_ms,
        cpu: CpuSample { tick_ms },
        task_ms: host.task_ms,
        gpu_ms: host.gpu_ms,
        draw_calls: host.draw_calls,
        triangles: host.triangles,
        buffers: host.buffers,
        buffer_mb: round(host.buffer_bytes / MB),
        textures_mb: host.textures_mb,
        census: host.census,
        px: host.px,
        mounts: MountSample {
            added: node_work.added as f64,
            removed: node_work.removed as f64,
            layouts: node_work.layouts as f64,
        },
        lod: LodSample {
            chunks: crate::world::chunk::node::chunk_nodes().len() as f64,
            pending: (pending_total - committed) as f64,
            in_frustum: census.in_frustum as f64,
            settling: census.settling as f64,
            blend_active: census.blend_active as f64,
            offscreen_pending: census.offscreen_pending as f64,
            oldest_release_ms: census.oldest_release_ms,
            oldest_visible_ms: census.oldest_visible_ms,
            pressure: census.pressure as f64,
        },
        tile_pages: tile_pages().keys().count() as f64,
        poi_count,
        heap: heap_sample(),
        rings: ledger_take_rings(),
        work: work_sample(c),
        q_terrain: fetch_pending_count(tile_fetcher::CATEGORY) as f64,
        q_poi: fetch_pending_count(fetcher::CATEGORY) as f64,
    });
}
