// pxl-kernel/turn.rs

//! Every layer's turn, in one place. The frame is one task: the effects,
//! then the tick's phases under one budget, then the draw. The other tasks
//! wait on what they work on.

#[path = "turn/step.spec.rs"]
pub mod spec;

use crate::domain::layer::Base as _;
use crate::sys::mem::section;
use pxl_spec::space::coord::q3::{self, Q3};
use spec::{EffectName, StepName};
use xpute_guest::sched::edge::{notified, Notify};

use crate::domain::control::camera::input::camera_input_step;
use crate::domain::control::navigation::bootstrap::map_bootstrap_step;
use crate::domain::control::navigation::follow_mode::follow_mode_step;
use crate::domain::control::navigation::off_center::{off_center, off_center_step};
use crate::domain::control::sensor::device_orientation::{orientation_watchdog_step, OrientationPlatform};
use crate::sys::io::request::{pump, set_credits};
use crate::sys::io::text::text_rise;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;

use crate::domain::layer::earth::Earth;
use crate::domain::query::decor::asset::decor_models_step;
use crate::domain::query::model::draw_list;
use crate::domain::scene::character::companion::{companion_step, CompanionProps};
use crate::domain::scene::environment::environment_step;
use crate::domain::scene::indicator::location_puck::location_puck_step;
use crate::domain::scene::scene;
use crate::domain::scene::stage::visible_step;
use crate::domain::view::demand::Observed as _;
use crate::profiler::report::{profiler_frame, FrameTurn};
use crate::sys::sched::edge::{request_world_tick, world_edge};
use crate::sys::sched::tick::{on_tick, run_tick, TickContext};
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
    if crate::domain::control::camera::controller::dist() == 0.0 {
        return None;
    }
    // Where the camera is going, not where its easing has got to: walked toward
    // only once the eased center settled, it set off as the glide's long tail ended.
    let goal = crate::domain::control::camera::controller::target_center().or_else(crate::domain::control::camera::controller::focus)?;
    let eye = crate::domain::view::frame::coord::world_to_canonical(&crate::domain::control::camera::controller::eye_world()?);
    let center = crate::domain::view::frame::coord::to_world(&goal);
    Some(CompanionProps {
        center: [center[0], center[1]],
        floor: crate::domain::scene::character::companion::focus_floor(goal, |w| crate::domain::control::navigation::support::floor_seen(w, &goal)),
        azimuth: crate::domain::control::camera::controller::azimuth(),
        dragging: crate::domain::control::camera::input::dragging(),
        steered: crate::domain::control::camera::input::steering(),
        following: crate::domain::control::navigation::follow_mode::following(),
        eye,
        tau: crate::domain::view::lens::tau(),
    })
}

/// The tool's ghost, drawn by the scene.
fn ghost_step() {
    use crate::domain::control::interaction::ghost::ghost;
    if section::map().is_some_and(|_| scene().ghost.set(draw_list(), &ghost())) {
        request_world_tick();
    }
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_props());
}

/// Each pyramid layer's tiles, from the live eye.
fn demand_tick(ctx: &mut TickContext) {
    // The ground's windows where their centres now stand, before what arrives fills them.
    let at = |q: Q3| [q3::to_real(i64::from(q[0])), q3::to_real(i64::from(q[1]))];
    let pinned = crate::domain::layer::terrain::window::step(
        [
            crate::domain::view::observation::eye().map(|e| [e[0], e[1]]),
            crate::domain::control::camera::controller::focus().map(at),
            crate::domain::control::navigation::destination::pending().map(at),
        ],
        pxl_render::gpu::api::device_generation(),
    );
    if pinned {
        TerrainTile::demand().moved();
    }
    crate::domain::view::demand::admit_arrived(&ctx.pass);
    changes_step();
    let tau = crate::domain::view::lens::tau();
    TerrainTile::demand().step(crate::domain::control::navigation::destination::tile(), crate::domain::layer::terrain::window::pinned(), tau);
    // Nothing is fetched while places are hidden.
    match crate::domain::view::setting::store::settings_store().place {
        true => PlaceTile::demand().step(None, &[], tau),
        false => PlaceTile::demand().idle(),
    }
    Earth::demand().step(None, &[], tau);
    DivisionTile::demand().step(crate::domain::layer::division::line(crate::domain::control::camera::controller::focus()), &[], tau);
}

/// The scene's turn, handed the camera's focus and, third person, its
/// heading, which the world gizmo is drawn about.
fn scene_tick(ctx: &mut TickContext) {
    let third_person = crate::domain::control::camera::controller::dist() != 0.0;
    visible_step(
        ctx,
        crate::domain::control::camera::controller::focus(),
        third_person.then(crate::domain::control::camera::controller::azimuth),
    );
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step(crate::domain::view::lens::tau());
}

/// The camera's last writer, then the view composed where it stands: after
/// every writer of the pose (the door's events, the turn's effects, the
/// controllers), before the phases that observe it.
fn camera_tick(ctx: &mut TickContext) {
    crate::domain::control::camera::controller::camera_step(ctx);
    crate::domain::view::frame::pose::view_compose(crate::domain::control::camera::controller::pose());
}

fn environment_tick(_: &mut TickContext) {
    environment_step();
}

/// Registers the tick's steps and spawns the tasks. Called from outside a
/// frame, since the frame task waits for the host's first frame.
pub fn boot() {
    on_tick(Phase::interaction, crate::domain::control::camera::controller::controller_step, StepName::controller as u32);
    on_tick(Phase::interaction, camera_input_tick, StepName::input as u32);
    on_tick(Phase::interaction, camera_tick, StepName::camera as u32);
    // After the view has moved, so the environment sees this tick's camera.
    on_tick(Phase::interaction, environment_tick, StepName::environment as u32);
    on_tick(Phase::interaction, companion_tick, StepName::companion as u32);
    on_tick(Phase::interaction, crate::domain::scene::character::preview::preview_step, StepName::preview as u32);
    on_tick(Phase::visible, demand_tick, StepName::demand as u32);
    on_tick(Phase::visible, scene_tick, StepName::scene as u32);
    world_tasks();
}

/// No more than edge.rs's TASKS; a refusal means the executor holds fewer, and aborts.
fn world_tasks() {
    use crate::sys::sched::edge::TaskPriority::*;
    use xpute_guest::sched::task::spawn;
    let spawned = spawn(frame_task(), Frame as u8) && spawn(crate::sys::io::fetch::fetch_task(), Arrival as u8) && spawn(crate::domain::layer::object::repository::task(), Background as u8);
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
    // First, so the turn they are read in is the first to see them.
    effect!(worst, settings, {
        if crate::domain::view::setting::store::settings_load_step() {
            crate::profiler::window::profiler_follow(crate::domain::view::setting::store::settings_store().profiler);
        }
    });
    effect!(worst, navigation, off_center_step());
    effect!(worst, bootstrap, map_bootstrap_step());
    effect!(worst, orientation, orientation_watchdog_step(platform.orientation));
    effect!(worst, follow, follow_mode_step());
    effect!(worst, destination, {
        // What the walker found under an unknown Z, let go where it moved.
        crate::domain::control::navigation::support::support_step();
        crate::domain::control::navigation::destination::destination_step()
    });
    effect!(worst, decor, decor_models_step());
    effect!(worst, objects, crate::domain::layer::object::repository::step());
    effect!(worst, edit, {
        let edits = crate::domain::control::interaction::edit::pending_step();
        // A new preview moves the world as drawn where it was and where it is.
        if crate::domain::layer::object::preview::set(edits).is_some() {
            request_world_tick();
        }
    });
    effect!(worst, hover, ghost_step());
    // device_coord is not gated like focus: the puck keeps its last coordinate across a transient none.
    effect!(worst, puck, location_puck_step(off_center().device_coord));
    effect!(worst, settings, crate::domain::view::setting::store::settings_keep_step());
    *WORST_EFFECT.get(|| (0.0, None)) = worst;
    now() - at
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
    pub delta: f64,
    /// The display's refresh, and the interval the host paces turns at, a
    /// whole number of refreshes: the frame's budget (xpute host
    /// sched/quantum.ts `Grant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
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
        let paced_ms = host.paced_ms;
        world_frame(host, edge.quota_ms(), edge.edge_at());
        crate::sys::sched::edge::tasks_after_frame(paced_ms);
        DRAWN.set(true);
    }
}

fn world_frame(host: HostFrame, quota_ms: f64, edge_at: f64) -> TickReport {
    let HostFrame {
        width,
        height,
        delta,
        refresh_ms,
        paced_ms,
        interactive,
        credits,
    } = host;
    let t0 = now();
    set_credits(credits);
    text_rise();
    let effects_ms = world_turn();
    // Before the tick, so every step and the draw see the screen it is drawn on.
    crate::domain::view::frame::pose::view_resize(width, height, crate::domain::view::frame::resolution::display_ratio());
    // Step by at most MAX_STEP_S, or a pause makes a character cross a wall or
    // a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = run_tick(step, quota_ms, edge_at);
    let after_tick = now();
    // The turn's one pump, after every producer has spoken: the effects, the
    // tick's tile streams (`demand_tick`), and the assets asked for by either.
    crate::sys::io::request::declare(crate::sys::io::demand::Stream::asset, crate::sys::io::loader::load_wants());
    pump();
    crate::domain::scene::stage::stage_write();
    crate::domain::scene::stage::stage_render(crate::domain::view::frame::resolution::Pace {
        paced_ms,
        gap_ms: delta * 1000.0,
        moving: interactive,
    });
    let done = now();
    profiler_frame(FrameTurn {
        tick: report,
        interval_ms: delta * 1000.0,
        refresh_ms,
        paced_ms,
        settled: !interactive,
        effects_ms,
        stage_ms: done - after_tick,
        task_ms: done - t0,
    });
    if report.invalidated {
        request_world_tick();
    }
    report
}

/// What moved this turn, told to what reads it: the tiles whose answer moved
/// (identities, not contents, which a tile's identity fixes) and where the
/// objects were written, the ground's held by the query
/// (domain/query/changed.rs). Taken after arrivals are admitted, before the
/// layers observe.
fn changes_step() {
    static MOVED: Global<[Vec<pxl_spec::space::layer::tile::Tile>; 3]> = Global::new();
    let [places, earth, divisions] = MOVED.get(Default::default);
    let shared = crate::domain::query::changed::take();
    crate::domain::scene::decor::drawn::edited(crate::domain::query::changed::written());
    PlaceTile::residency().take_moved(places);
    Earth::residency().take_moved(earth);
    DivisionTile::residency().take_moved(divisions);
    if shared {
        TerrainTile::demand().moved();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().moved();
        crate::domain::scene::marker::moved(places);
    }
    if shared || !earth.is_empty() {
        Earth::demand().moved();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().moved();
    }
}
