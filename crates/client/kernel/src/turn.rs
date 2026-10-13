// pxl-kernel/turn.rs

//! Every layer's turn, in one place. The frame is one task: the effects,
//! then the tick's phases under one budget, then the draw. The other tasks
//! wait on what they work on.

#[path = "turn/step.spec.rs"]
pub mod spec;

use crate::layer::Base as _;
use pxl_spec::space::coord::aabb::Aabb;
use spec::{EffectName, StepName};
use xpute_guest::sched::edge::{notified, Notify};

use crate::control::camera::input::camera_input_step;
use crate::control::navigation::bootstrap::map_bootstrap_step;
use crate::control::navigation::follow_mode::follow_mode_step;
use crate::control::navigation::off_center::{off_center, off_center_step};
use crate::control::sensor::device_orientation::{orientation_watchdog_step, OrientationPlatform};
use crate::control::sensor::geolocation::geolocation_step;
use crate::io::request::{pump, set_credits};
use crate::io::text::text_rise;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;

use crate::decor::assets::decor_assets_wanted;
use crate::kernel;
use crate::layer::earth::Earth;
use crate::profiler::report::{report_step, set_host_task_ms, set_turn_timing};
use crate::scene::character::companion::{companion_step, CompanionProps};
use crate::scene::environment::environment_step;
use crate::scene::indicator::frame_gizmo::frame_gizmo_step;
use crate::scene::indicator::location_puck::location_puck_step;
use crate::scene::stage::{take_draw, visible_step};
use crate::sched::edge::{request_world_tick, world_edge};
use crate::sched::tick::{on_tick, run_tick, TickContext, TickState};
use crate::setting::store::settings_store;
use crate::view::demand::Observed as _;
use pxl_spec::bake::tile::division::DivisionTile;
use xpute_guest::clock::now;
use xpute_guest::global::Global;
use xpute_guest::sched::tick::{Phase, TickReport};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub geolocation: bool,
    pub orientation: OrientationPlatform,
}

/// None while the eye is the focus: then the character is the one looking.
fn companion_props() -> Option<CompanionProps> {
    if crate::control::camera::controller::dist() == 0.0 {
        return None;
    }
    // Where the camera is going, not where its easing has got to: walked toward
    // only once the eased center settled, it set off as the glide's long tail ended.
    let goal = crate::control::camera::controller::target_center().or_else(crate::control::camera::controller::focus)?;
    let eye = crate::view::frame::coord::world_to_canonical(&crate::control::camera::controller::eye_world()?);
    let center = crate::view::frame::coord::to_world(&goal);
    Some(CompanionProps {
        center: [center[0], center[1]],
        floor: crate::control::navigation::support::floor_seen(&goal),
        azimuth: crate::control::camera::controller::azimuth(),
        dragging: crate::control::camera::state::camera_state().camera_status == crate::control::camera::state::CameraStatus::Drag,
        steered: crate::control::camera::input::steering(),
        following: crate::control::navigation::follow_mode::following(),
        eye,
        tau: crate::view::sight::effective(),
    })
}

/// The tool's ghost, drawn by the scene.
fn ghost_step() {
    use crate::control::interaction::ghost::{ghost, Ghost};
    let changed = kernel().parts().map(|(_, _, scene, draw)| match ghost() {
        Ghost::None => scene.ghost.clear(draw),
        Ghost::Decor { at, group, rotation, scale } => scene.ghost.set_decor(draw, at, group, rotation as f32, scale as f32),
        Ghost::Box { at, size, value, rgb, coplanar } => scene.ghost.set_box(draw, at, size, value, rgb, coplanar),
    });
    if changed == Some(true) {
        request_world_tick();
    }
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_props());
}

fn world_gizmo_tick(_: &mut TickContext) {
    let focus = crate::control::camera::controller::focus();
    let show = settings_store().world_gizmo && focus.is_some();
    let third_person = crate::control::camera::controller::dist() != 0.0;
    frame_gizmo_step(show, focus, third_person.then(crate::control::camera::controller::azimuth));
}

/// A new cast, where what it observes moved.
fn sight_tick(_: &mut TickContext) {
    crate::view::sight::start();
}

/// A slice of the goal walk, then quality. An unfinished walk asks for another turn.
fn goal_tick(ctx: &mut TickContext) {
    let end = now() + ctx.budget.remaining() * crate::param::VIEW_SHARE;
    crate::view::sight::advance(&|| now() >= end);
    if crate::view::sight::casting() {
        ctx.invalidate();
    }
}

/// Each pyramid layer's tiles, from the published goal.
fn demand_tick(_: &mut TickContext) {
    crate::view::demand::admit_arrived();
    changes_step();
    // Demand follows the goal the chunks published, at the quality decided first.
    let published = crate::view::sight::current().and_then(|s| {
        Some(crate::view::demand::Published {
            observation: s.observation()?,
            stops: s.stops(),
            lacks: s.lacks(),
        })
    });
    let tau = crate::view::sight::effective();
    TerrainTile::demand().step(crate::control::navigation::destination::tile(), published, tau);
    // Nothing is fetched while places are hidden.
    match crate::setting::store::settings_store().place {
        true => PlaceTile::demand().step(None, published, tau),
        false => PlaceTile::demand().idle(),
    }
    Earth::demand().step(None, published, tau);
    DivisionTile::demand().step(crate::layer::division::line(crate::control::camera::controller::focus()), published, tau);
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step(crate::view::sight::effective());
}

fn environment_tick(_: &mut TickContext) {
    environment_step();
}

/// Registers the tick's steps and spawns the tasks. Called from outside a
/// frame, since the frame task waits for the host's first frame.
pub fn boot() {
    on_tick(Phase::interaction, crate::control::camera::controller::controller_step, StepName::controller as u32);
    on_tick(Phase::interaction, camera_input_tick, StepName::input as u32);
    on_tick(Phase::interaction, crate::control::camera::controller::camera_step, StepName::camera as u32);
    // After the view has moved, so the environment sees this tick's camera.
    on_tick(Phase::interaction, environment_tick, StepName::environment as u32);
    on_tick(Phase::interaction, companion_tick, StepName::companion as u32);
    on_tick(Phase::interaction, crate::scene::character::preview::preview_step, StepName::preview as u32);
    on_tick(Phase::visible, sight_tick, StepName::sight as u32);
    on_tick(Phase::visible, goal_tick, StepName::goal as u32);
    on_tick(Phase::visible, demand_tick, StepName::demand as u32);
    on_tick(Phase::visible, visible_step, StepName::chunks as u32);
    on_tick(Phase::cosmetic, world_gizmo_tick, StepName::world_gizmo as u32);
    on_tick(Phase::report, report_step, StepName::profiler as u32);
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

/// No more than edge.rs's TASKS; a refusal means the executor holds fewer, and aborts.
fn world_tasks() {
    use xpute_guest::sched::task::spawn;
    use TaskPriority::*;
    let spawned = spawn(frame_task(), Frame as u8)
        && spawn(crate::io::fetch::fetch_task(), Arrival as u8)
        && spawn(crate::io::load::gltf_task(), Arrival as u8)
        && spawn(crate::chunk::build::build_task(), Build as u8)
        && spawn(crate::decor::assets::decor_asset_task(), Background as u8)
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
    effect!(worst, navigation, off_center_step());
    effect!(worst, bootstrap, map_bootstrap_step());
    effect!(worst, geolocation, geolocation_step(platform.geolocation));
    effect!(worst, orientation, orientation_watchdog_step(platform.orientation));
    effect!(worst, follow, follow_mode_step());
    effect!(worst, destination, crate::control::navigation::destination::destination_step());
    // After every writer of the focus, before anything observes the view.
    effect!(worst, view, crate::view::frame::pose::view_compose(crate::control::camera::controller::pose()));
    effect!(worst, decor, decor_assets_wanted());
    effect!(worst, objects, crate::layer::object::repository::step());
    effect!(worst, edit, {
        let edits = crate::control::interaction::edit::pending_step();
        // A new preview moves the world as drawn where it was and where it is.
        if let Some(was) = crate::layer::object::preview::set(edits) {
            crate::chunk::source::chunk_sources_changed();
            crate::view::sight::moved(was.cubes().chain(edits.cubes()));
            request_world_tick();
        }
    });
    effect!(worst, hover, ghost_step());
    // device_coord is not gated like focus: the puck keeps its last coordinate across a transient none.
    effect!(worst, puck, location_puck_step(off_center().device_coord));
    effect!(worst, settings, crate::setting::store::settings_keep_step());
    // After every producer has spoken, including assets asked for during the turn.
    effect!(worst, assets, crate::io::request::declare(crate::io::demand::Stream::asset, crate::io::load::load_wants()));
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
    crate::view::frame::resolution::resolution_step(state);
    crate::scene::stage::stage_write();
    crate::scene::stage::stage_render();
    let done = now();
    set_turn_timing(effects_ms, done - after_tick);
    set_host_task_ms(done - t0);
    if report.invalidated || take_draw() {
        request_world_tick();
    }
    report
}

/// What moved this turn, told to what reads it: the tiles whose answer moved
/// (identities, not contents, which a tile's identity fixes) and where the
/// objects were written. Taken after arrivals are admitted, before the
/// layers observe and the chunks compare their sources.
fn changes_step() {
    static MOVED: Global<[Vec<pxl_spec::space::layer::tile::Tile>; 4]> = Global::new();
    static WRITTEN: Global<Vec<Aabb>> = Global::new();
    let [ground, places, earth, divisions] = MOVED.get(Default::default);
    let written = WRITTEN.get(Vec::new);
    TerrainTile::residency().take_moved(ground);
    PlaceTile::residency().take_moved(places);
    Earth::residency().take_moved(earth);
    DivisionTile::residency().take_moved(divisions);
    crate::layer::object::store::take_written(written);
    let shared = !ground.is_empty() || !written.is_empty();
    if !written.is_empty() {
        crate::chunk::source::objects_written(written);
    }
    if shared {
        crate::chunk::source::chunk_sources_changed();
        crate::view::sight::moved(ground.iter().map(|p| p.region()).chain(written.iter().copied()));
        TerrainTile::demand().moved();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().moved();
        crate::chunk::place::moved();
    }
    if shared || !earth.is_empty() {
        Earth::demand().moved();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().moved();
    }
}
