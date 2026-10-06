// pxl-kernel/sched/turn.rs

//! Every layer's turn, in one place. The frame is one task: the effects,
//! then the tick's phases under one budget, then the draw. The other tasks
//! wait on what they work on.

use xpute_runtime::sched::edge::{notified, Notify};

use crate::control::camera::input::camera_input_step;
use crate::control::navigation::bootstrap::map_bootstrap_step;
use crate::control::navigation::follow_mode::follow_mode_step;
use crate::control::navigation::store::{map_navigation, map_navigation_step};
use crate::control::sensor::device_orientation::{orientation_watchdog_step, OrientationPlatform};
use crate::control::sensor::geolocation::geolocation_step;
use crate::io::sched::{pump, set_credits};
use crate::io::text::text_rise;
use crate::layer::geocode::fetcher::geocode_fetcher_step;
use pxl_spec::bake::format::Terrain;
use pxl_spec::bake::poi_tile::PoiTile;

use crate::layer::earth::Earth;
use crate::scene::character::companion::{companion_step, CompanionProps};
use crate::scene::environment::state::map_environment_step;
use crate::scene::marker::frame_gizmo::debug_gizmos_step;
use crate::scene::marker::hover_cursor::hover_cursor_step;
use crate::scene::marker::location_puck::location_puck_step;
use crate::sched::profiler::spec::EffectName;
use crate::sched::tick::{on_tick, run_tick, StepName, TickContext, TickState};
use crate::shell::setting::store::settings_store;
use crate::world::change::{request_world_tick, world_edge};
use crate::world::chunk::decor::asset_store::decor_assets_wanted;
use crate::world::chunk::source::chunk_sources_step;
use crate::world::chunk::stage::{core_tick_report_step, core_tick_visible_step, set_host_task_ms, set_turn_timing, take_draw};
use crate::world::pyramid::Observed as _;
use xpute_runtime::clock::now;
use xpute_runtime::global::Global;
use xpute_runtime::sched::tick::{TickReport, COSMETIC, INTERACTION, REPORT, VISIBLE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub geolocation: bool,
    pub orientation: OrientationPlatform,
}

/// None while the eye is the map center: then the character is the one looking.
fn companion_props() -> Option<CompanionProps> {
    if crate::control::camera::controller::dist() == 0.0 {
        return None;
    }
    let map_center = crate::control::camera::controller::map_center()?;
    let center = crate::world::view::frame::coord::to_world(&map_center);
    Some(CompanionProps {
        center: [center[0], center[1]],
        floor: crate::control::navigation::support::floor_of(&map_center),
    })
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_props());
}

fn debug_gizmos_tick(_: &mut TickContext) {
    let map_center = crate::control::camera::controller::map_center();
    let show = settings_store().gizmos && map_center.is_some();
    debug_gizmos_step(show, map_center);
}

/// The rays a new goal starts from.
fn sight_tick(_: &mut TickContext) {
    crate::world::chunk::quality::sight();
}

/// A slice of the goal walk, then quality. An unfinished walk asks for another turn.
fn goal_tick(ctx: &mut TickContext) {
    let end = now() + ctx.budget.remaining() * crate::param::VIEW_SHARE;
    crate::world::chunk::quality::step(&|| now() >= end);
    if crate::world::chunk::domain::walking() {
        ctx.invalidate();
    }
}

/// Each pyramid layer's tiles, from the published goal.
fn demand_tick(_: &mut TickContext) {
    Terrain::pyramid().step(crate::control::navigation::destination::tile());
    // Nothing is fetched while places are hidden.
    match crate::layer::poi::enabled() {
        true => PoiTile::pyramid().step(None),
        false => PoiTile::pyramid().idle(),
    }
    Earth::pyramid().step(None);
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step();
}

fn environment_tick(_: &mut TickContext) {
    map_environment_step();
}

/// Registers the tick's steps and spawns the tasks. Called from outside a
/// frame, since the frame task waits for the host's first frame.
pub fn boot() {
    on_tick(INTERACTION, crate::control::camera::controller::controller_step, StepName::controller);
    on_tick(INTERACTION, camera_input_tick, StepName::input);
    on_tick(INTERACTION, crate::control::camera::controller::camera_step, StepName::camera);
    // After the view has moved, so the environment sees this tick's camera.
    on_tick(INTERACTION, environment_tick, StepName::environment);
    on_tick(INTERACTION, companion_tick, StepName::companion);
    on_tick(INTERACTION, crate::scene::character::preview::preview_step, StepName::preview);
    on_tick(VISIBLE, sight_tick, StepName::sight);
    on_tick(VISIBLE, goal_tick, StepName::goal);
    on_tick(VISIBLE, demand_tick, StepName::demand);
    on_tick(VISIBLE, core_tick_visible_step, StepName::chunks);
    on_tick(COSMETIC, debug_gizmos_tick, StepName::debug_gizmos);
    on_tick(REPORT, core_tick_report_step, StepName::profiler);
    world_tasks();
}

/// The executor may ignore this order; nothing relies on it.
#[repr(u8)]
#[derive(Clone, Copy)]
enum TaskPriority {
    Frame,
    Arrival,
    Build,
    Background,
}

/// No more than change.rs's TASKS; a refusal means the executor holds fewer, and aborts.
fn world_tasks() {
    use xpute_runtime::sched::task::spawn;
    use TaskPriority::*;
    let spawned = spawn(frame_task(), Frame as u8)
        && spawn(crate::io::fetch::fetch_task(), Arrival as u8)
        && spawn(crate::io::load::gltf_task(), Arrival as u8)
        && spawn(crate::world::chunk::stage::build_task(), Build as u8)
        && spawn(crate::world::chunk::decor::asset_store::decor_asset_task(), Background as u8)
        && spawn(crate::layer::object::repository::task(), Background as u8);
    if !spawned {
        std::process::abort();
    }
}

/// Longest effect of the last turn and its EFFECT_NAMES index: a turn is only
/// as cooperative as its longest step.
static WORST_EFFECT: Global<(f64, Option<u32>)> = Global::new();

pub fn worst_effect() -> (f64, Option<u32>) {
    *WORST_EFFECT.get(|| (0.0, None))
}

/// Effects are not budgeted, so nothing in them may be real work. They were
/// untimed until a fold here took 70 ms of a turn whose tick was 1.5 ms
/// (2026-09-23).
macro_rules! effect {
    ($worst:ident, $name:ident, $step:expr) => {{
        let at = now();
        $step;
        let ms = now() - at;
        if ms > $worst.0 {
            $worst = (ms, Some(EffectName::$name as u32));
        }
    }};
}

/// Returns how long it took.
pub fn world_turn() -> f64 {
    let at = now();
    let platform = world_platform();
    let mut worst = (0.0f64, None);
    effect!(worst, navigation, map_navigation_step());
    effect!(worst, bootstrap, map_bootstrap_step());
    effect!(worst, geolocation, geolocation_step(platform.geolocation));
    effect!(worst, orientation, orientation_watchdog_step(platform.orientation));
    effect!(worst, follow, follow_mode_step());
    effect!(worst, destination, crate::control::navigation::destination::destination_step());
    // After every writer of the map center, before anything observes the view.
    effect!(worst, view, crate::world::view::frame::pose::view_compose());
    effect!(worst, geocode, geocode_fetcher_step());
    effect!(worst, decor, decor_assets_wanted());
    effect!(worst, chunk_sources, chunk_sources_step());
    effect!(worst, hover, hover_cursor_step());
    // device_coord is not gated like map_center: the puck keeps its last coordinate across a transient none.
    effect!(worst, puck, location_puck_step(map_navigation().device_coord));
    effect!(worst, objects, crate::layer::object::repository::step());
    effect!(worst, settings, crate::shell::setting::store::settings_keep_step());
    // After every producer has spoken, including assets asked for during the turn.
    effect!(worst, assets, crate::io::sched::declare(crate::io::demand::Category::asset, crate::io::load::load_wants()));
    effect!(worst, pump, pump());
    *WORST_EFFECT.get(|| (0.0, None)) = worst;
    now() - at
}

static TICK_STATE: Global<TickState> = Global::new();

/// Whether the last turn was interactive. Read by steps that run before the
/// tick sets it, so it lags a turn, which is fine for deciding whether to wait.
pub fn interactive() -> bool {
    TICK_STATE.get(TickState::default).interactive
}
static PLATFORM: Global<Option<Platform>> = Global::new();

pub fn set_world_platform(platform: Platform) {
    *PLATFORM.get(|| None) = Some(platform);
}

pub fn world_platform() -> Platform {
    PLATFORM.get(|| None).unwrap_or(Platform {
        geolocation: false,
        orientation: OrientationPlatform {
            supported: false,
            needs_permission: false,
            silence_is_loss: false,
        },
    })
}

/// Longest gap still stepped as motion; past it (a hidden tab, a stall) the
/// world holds still rather than leaping.
const MAX_STEP_S: f64 = 0.1;

pub struct HostFrame {
    pub width: f64,
    pub height: f64,
    pub dpr: f64,
    pub delta: f64,
    pub frame_ms: f64,
    pub interactive: bool,
    pub credits: u32,
}

static FRAME: Global<Option<HostFrame>> = Global::new();
static FRAMES: Global<Notify> = Global::new();
static DRAWN: Global<bool> = Global::new();

fn frames() -> &'static mut Notify {
    FRAMES.get(Notify::new)
}

pub fn world_frame_asked(host: HostFrame) {
    *FRAME.get(|| None) = Some(host);
    frames().notify();
}

/// A turn carrying a frame polls the tasks whatever its quota, so an overrun quota still draws.
pub fn world_frame_waiting() -> bool {
    FRAME.get(|| None).is_some()
}

pub fn take_world_drawn() -> bool {
    core::mem::take(DRAWN.get(|| false))
}

/// One task, because the frame's parts have an order and the executor's
/// order among tasks is never relied on.
async fn frame_task() {
    loop {
        notified(frames).await;
        let Some(host) = FRAME.get(|| None).take() else { continue };
        let edge = world_edge();
        world_frame(host, edge.quota_ms(), edge.edge_at());
        DRAWN.set(true);
    }
}

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
    set_credits(credits);
    text_rise();
    let effects_ms = world_turn();
    let state = TICK_STATE.get(TickState::default);
    state.width = width;
    state.height = height;
    state.dpr = dpr;
    // Step by at most MAX_STEP_S, or a pause makes a character cross a wall or
    // a damped camera overshoot. The raw gap is still recorded, or every bad
    // frame would read as exactly the clamp.
    let step = delta.min(MAX_STEP_S);
    state.gap_s = delta;
    state.elapsed_s += step;
    state.frame_ms = frame_ms;
    state.interactive = interactive;
    let report = run_tick(state, step, quota_ms, edge_at);
    let after_tick = now();
    pump();
    crate::scene::stage::stage_write(crate::world::view::frame::resolution::resolution_step(state));
    crate::scene::stage::stage_render();
    let done = now();
    set_turn_timing(effects_ms, done - after_tick);
    set_host_task_ms(done - t0);
    if report.invalidated || take_draw() {
        request_world_tick();
    }
    report
}
