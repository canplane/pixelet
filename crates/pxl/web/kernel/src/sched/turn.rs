// pxl-kernel/sched/turn.rs

//! The world as the kernel runs it: every layer's turn, in one place.
//!
//! Coordinate model: every layer places what it has in the root frame
//! (world/frame/view.rs), the map's azimuth and the floating origin's
//! translation, which the view's damping steps write.
//!
//! The kernel's work is tasks on whichever executor the module installed
//! (`world_tasks`). The frame is one of them: the effects — the fetchers, the
//! stores' followers, the markers — each comparing what it last saw
//! (`world_turn`), then the tick's steps (sched/tick.rs) by phase under one
//! budget, then the draw. The rest wait on what they work on: arrivals, the
//! chunk builds, the decor models, the save.

use pxl_spec::space::indexer::L3_IDX;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::xy::TO_XY;
use xpute_runtime::sched::edge::{notified, Notify};

use crate::control::camera::damping::camera_step;
use crate::control::camera::input::camera_input_step;
use crate::control::navigation::bootstrap::map_bootstrap_step;
use crate::control::navigation::follow_mode::follow_mode_step;
use crate::control::navigation::store::{map_navigation, map_navigation_step};
use crate::control::sensor::device_orientation::{orientation_watchdog_step, OrientationPlatform};
use crate::control::sensor::geolocation::geolocation_step;
use crate::global::Global;
use crate::host::now;
use crate::io::scheduler::{fetch_pump, set_fetch_credits};
use crate::layer::character::companion::{companion_step, CompanionProps};
use crate::layer::decor::asset_store::decor_assets_wanted;
use crate::layer::environment::state::map_environment_step;
use crate::layer::environment::theme_clock::theme_clock_step;
use crate::layer::marker::frame_gizmo::debug_gizmos_step;
use crate::layer::marker::hover_cursor::hover_cursor_step;
use crate::layer::marker::location_puck::location_puck_step;
use crate::layer::object::teach::teach_objects;
use crate::layer::terrain::teach::teach_terrain;
use crate::layer::terrain::tile_fetcher::tile_fetcher_step;
use crate::sched::tick::{on_tick, run_tick, TickContext, TickReport, TickState, COSMETIC, INTERACTION, REPORT, VISIBLE};
use crate::shell::config::env::APP_ENV;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::settings_store;
use crate::world::change::{request_world_tick, world_edge};
use crate::world::chunk::node::chunk_nodes_step;
use crate::world::chunk::stage::{core_tick_report_step, core_tick_visible_step, set_host_task_ms, set_turn_timing, take_draw};
use crate::world::frame::coord::local_world_origin;
use crate::world::frame::view_frame::{view_rotation_step, view_translation_step};

/// What the platform offers the sensors, as the shell found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub geolocation: bool,
    pub orientation: OrientationPlatform,
}

/// The companion's place, from map_center in the local floating-origin frame —
/// wherever the camera actually is, not the origin's cell's centre.
fn companion_props() -> Option<CompanionProps> {
    let map_center = map_store().map_center?;
    let map_origin = local_world_origin();
    let mut local = [0.0; 3];
    projection::unproject(&mut local, ivec::sub(map_center.i, map_origin), map_center.f);
    Some(CompanionProps {
        origin_page_idx_xy: TO_XY(L3_IDX(map_origin)),
        map_origin,
        center_x: local[0],
        center_y: local[1],
        center_z: local[2],
    })
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_props());
}

fn debug_gizmos_tick(_: &mut TickContext) {
    let map_center = map_store().map_center;
    let show = APP_ENV() != "production" && settings_store().show_debug_objects && map_center.is_some();
    debug_gizmos_step(show, map_center, local_world_origin());
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step();
}

fn environment_tick(_: &mut TickContext) {
    map_environment_step();
}

/// The kernel's boot, once, from the module's: the build taught its tables —
/// before the first page is told — the tick's steps registered, and the tasks
/// spawned. Nothing runs until the first poll; the frame task waits for the
/// first frame the host asks, so the boot has to come from outside a frame.
pub fn boot() {
    teach_terrain();
    teach_objects();

    on_tick(INTERACTION, camera_input_tick, "input");
    on_tick(INTERACTION, camera_step, "camera");
    on_tick(INTERACTION, view_rotation_step, "view_rotation");
    on_tick(INTERACTION, view_translation_step, "view_translation");
    // After the view has moved: the environment observes the camera centre
    // the view eased to this tick.
    on_tick(INTERACTION, environment_tick, "environment");
    on_tick(INTERACTION, companion_tick, "companion");
    on_tick(INTERACTION, crate::layer::character::preview::preview_step, "preview");
    on_tick(VISIBLE, core_tick_visible_step, "chunks");
    on_tick(COSMETIC, debug_gizmos_tick, "debug_gizmos");
    on_tick(REPORT, core_tick_report_step, "ledger");
    world_tasks();
}

/// How soon each of the kernel's tasks wants polling, lower sooner. The
/// installed executor may not order by it; nothing here rests on its order.
#[repr(u8)]
#[derive(Clone, Copy)]
enum TaskPriority {
    Frame,
    Arrival,
    Build,
    Background,
}

/// Spawns the kernel's tasks on whichever executor the module installed —
/// no more than change.rs's TASKS. One refused is a module whose executor
/// holds fewer, which aborts.
fn world_tasks() {
    use xpute_runtime::sched::task::spawn;
    use TaskPriority::*;
    let spawned = spawn(frame_task(), Frame as u8)
        && spawn(crate::io::fetch::fetch_task(), Arrival as u8)
        && spawn(crate::io::load::gltf_task(), Arrival as u8)
        && spawn(crate::layer::terrain::tile_fetcher::fold_task(), Arrival as u8)
        && spawn(crate::world::chunk::stage::build_task(), Build as u8)
        && spawn(crate::layer::decor::asset_store::decor_asset_task(), Background as u8)
        && spawn(crate::layer::object::store::save_task(), Background as u8);
    if !spawned {
        std::process::abort();
    }
}

/// The longest effect of the last turn, and which it was: a turn is only as
/// cooperative as its longest step, and a total does not say where to look.
/// Named as EFFECT_NAMES spells it (sched/ledger/ledger.json).
static WORST_EFFECT: Global<(f64, &'static str)> = Global::new();

pub fn worst_effect() -> (f64, &'static str) {
    *WORST_EFFECT.get(|| (0.0, ""))
}

/// One effect, timed. The effects are where the turn decides what is wanted,
/// and they are not budgeted — nothing here may be work. What kept that
/// honest was nothing at all until a fold running here took seventy
/// milliseconds of a turn whose tick was one and a half, and no number in
/// the ledger named the region it was in (2026-09-23).
macro_rules! effect {
    ($worst:ident, $name:literal, $step:expr) => {{
        let at = now();
        $step;
        let ms = now() - at;
        if ms > $worst.0 {
            $worst = (ms, $name);
        }
    }};
}

/// The effects' turn, before the tick. How long it took.
pub fn world_turn() -> f64 {
    let at = now();
    let platform = world_platform();
    let mut worst = (0.0f64, "");
    effect!(worst, "navigation", map_navigation_step());
    effect!(worst, "bootstrap", map_bootstrap_step());
    effect!(worst, "shell", crate::shell::shell::shell_step());
    effect!(worst, "geolocation", geolocation_step(platform.geolocation));
    effect!(worst, "orientation", orientation_watchdog_step(platform.orientation));
    effect!(worst, "tiles", tile_fetcher_step());
    effect!(worst, "follow", follow_mode_step());
    effect!(worst, "theme", theme_clock_step());
    effect!(worst, "decor", decor_assets_wanted());
    effect!(worst, "chunk_nodes", chunk_nodes_step());
    effect!(worst, "hover", hover_cursor_step());
    // device_coord is deliberately not gated the way map_center is — the puck
    // holds its own last-known coordinate across a transient none.
    effect!(worst, "puck", location_puck_step(map_navigation().device_coord));
    effect!(worst, "objects", crate::layer::object::store::flush_save_if_due());
    effect!(worst, "pump", fetch_pump());
    *WORST_EFFECT.get(|| (0.0, "")) = worst;
    now() - at
}

static TICK_STATE: Global<TickState> = Global::new();

/// Whether the host ran the last turn as interactive — the view is being
/// moved rather than looked at. Read by a turn's own steps, which run before
/// the tick sets it, so it is the turn before's answer; what reads it is
/// deciding whether to act now or wait for the view to settle, and one turn
/// of lag there is nothing.
pub fn interactive() -> bool {
    TICK_STATE.get(TickState::default).interactive
}
static PLATFORM: Global<Option<Platform>> = Global::new();

/// Tells the kernel the platform, once, at boot: what a device offers does not
/// change while the page runs.
pub fn set_world_platform(platform: Platform) {
    *PLATFORM.get(|| None) = Some(platform);
}

/// The platform as the boot told it.
pub fn world_platform() -> Platform {
    PLATFORM.get(|| None).unwrap_or(Platform {
        geolocation: false,
        orientation: OrientationPlatform {
            supported: false,
            needs_permission: false,
        },
    })
}

/// What the host tells a frame: the viewport, the gap since the last, and how
/// it sees the turn.
pub struct HostFrame {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    pub delta: f64,
    pub frame_ms: f64,
    pub interactive: bool,
    /// The fetches the kernel may have in flight.
    pub credits: u32,
}

static FRAME: Global<Option<HostFrame>> = Global::new();
static FRAMES: Global<Notify> = Global::new();
static DRAWN: Global<bool> = Global::new();

fn frames() -> &'static mut Notify {
    FRAMES.get(Notify::new)
}

/// The host asked for a frame: kept for the frame task, the latest winning.
pub fn world_frame_asked(host: HostFrame) {
    *FRAME.get(|| None) = Some(host);
    frames().notify();
}

/// Whether the host asked for a frame not yet drawn: a turn that carries one
/// polls the tasks, whatever its quota — a quota overrun to nothing still
/// draws.
pub fn world_frame_waiting() -> bool {
    FRAME.get(|| None).is_some()
}

/// Whether a frame was drawn since the last take: what the module reports
/// back to the shell after the turn.
pub fn take_world_drawn() -> bool {
    core::mem::take(DRAWN.get(|| false))
}

/// The frame task: a frame the host asked for, whole. What a frame does has an
/// order within the turn — the effects, the tick's phases, the fetch pump,
/// the draw — so it is one task, and the executor's order among tasks is
/// never relied on.
async fn frame_task() {
    loop {
        notified(frames).await;
        let Some(host) = FRAME.get(|| None).take() else { continue };
        let edge = world_edge();
        world_frame(host, edge.quota_ms(), edge.edge_at());
        DRAWN.set(true);
    }
}

/// The frame of the turn the central scheduler strobed at `edge_at` with
/// `quota_ms`: the effects, then the tick's phases on what is left of the
/// quota, the fetches they asked for within the credits, and the draw. A pass
/// that asked for a turn after this one, or left something new to draw, asks
/// for the next edge.
fn world_frame(host: HostFrame, quota_ms: f64, edge_at: f64) -> TickReport {
    let HostFrame {
        width,
        height,
        dpr,
        delta,
        frame_ms,
        interactive,
        credits,
    } = host;
    let t0 = now();
    set_fetch_credits(credits);
    let effects_ms = world_turn();
    let state = TICK_STATE.get(TickState::default);
    state.width = width;
    state.height = height;
    state.dpr = dpr;
    state.elapsed_s += delta;
    state.frame_ms = frame_ms;
    state.interactive = interactive;
    let report = run_tick(state, delta, quota_ms, edge_at);
    let after_tick = now();
    fetch_pump();
    crate::world::frame::stage::stage_write();
    crate::world::frame::stage::stage_render();
    let done = now();
    set_turn_timing(effects_ms, done - after_tick);
    set_host_task_ms(done - t0);
    if report.invalidated || take_draw() {
        request_world_tick();
    }
    report
}
