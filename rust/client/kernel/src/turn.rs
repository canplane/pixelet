// pxl-kernel/turn.rs

//! Every layer's turn, in one place. The frame is one task: the effects,
//! then the tick's phases under one budget, then the draw. The other tasks
//! wait on what they work on.

#[path = "turn/step.spec.rs"]
pub mod spec;

use crate::domain::layer::Base as _;
use crate::sys::boot;
use pxl_spec::space::coord::q3::{self, Q3};
use spec::{EffectName, StepName};
use xpute_core::sched::edge::{notified, Notify};

use crate::domain::control::input::{camera_input_step, rehover_at};
use crate::domain::control::navigation::bootstrap::map_bootstrap_step;
use crate::domain::control::navigation::follow_mode::follow_mode_step;
use crate::domain::control::navigation::off_center::{off_center, off_center_step};
use crate::domain::control::sensor::device_orientation::orient_watchdog_step;
use crate::door::cmd;
use crate::sys::io::request::{pump, set_credits};
use crate::sys::io::text::text_rise;
use pxl_spec::bake::tile::place::PlaceTile;
use pxl_spec::bake::tile::terrain::TerrainTile;

use crate::domain::control::camera;
use crate::domain::control::camera::controller;
use crate::domain::control::camera::controller::Active;
use crate::domain::control::edit::write;
use crate::domain::control::input;
use crate::domain::control::navigation::destination;
use crate::domain::control::navigation::follow_mode;
use crate::domain::control::support;
use crate::domain::layer::division;
use crate::domain::layer::earth::Earth;
use crate::domain::layer::object::repository;
use crate::domain::layer::terrain::clipmap;
use crate::domain::query::changes;
use crate::domain::scene::character::companion::{self, companion_step, Camera};
use crate::domain::scene::decor::asset::decor_models_step;
use crate::domain::scene::decor::gpu;
use crate::domain::scene::environment::environment_step;
use crate::domain::scene::indicator::ghost::ghost_set;
use crate::domain::scene::indicator::location::location_step;
use crate::domain::scene::marker;
use crate::domain::scene::model::draw_list;
use crate::domain::scene::resolution::Pace;
use crate::domain::scene::stage;
use crate::domain::scene::stage::visible_step;
use crate::domain::view::demand;
use crate::domain::view::demand::Observed as _;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::domain::view::setting::store;
use crate::perf::record::perf_recording;
use crate::perf::report::{perf_frame, set_renderer_frame, FrameTurn};
use crate::perf::window::perf_follow;
use crate::pointer;
use crate::sys::io::demand::Stream;
use crate::sys::io::fetch;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::sched::edge;
use crate::sys::sched::edge::{request_world_tick, world_edge};
use crate::sys::sched::tick::{on_tick, run_tick, TickContext};
use pxl_render::gpu::api;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::sched::tick::TickReport;

use crate::sys::sched::phase_spec::Phase;

/// The camera as the companion's walk reads it; none while the eye is the
/// pivot, when the person is the one looking.
fn companion_camera() -> Option<Camera> {
    if controller::active() != Active::Orbit {
        return None;
    }
    // Where the camera is going, not where its easing has got to.
    let goal = controller::pivot_to().or_else(controller::pivot)?;
    let eye = coord::world_to_canonical(&camera::eye_world()?);
    let at = coord::to_world(&goal);
    Some(Camera {
        pivot: [at[0], at[1]],
        floor: companion::pivot_floor(goal, |w| support::floor_at(w, &goal)),
        azimuth: camera::azimuth(),
        dragging: input::dragging(),
        steered: input::steering(),
        following: follow_mode::following(),
        eye,
    })
}

/// The edit the pointer would make, of what it is over where the camera now
/// stands (`camera_tick`), so the scene draws it this turn, not the next.
fn edit_tick(_: &mut TickContext) {
    // A new preview moves the world as drawn where it was and where it is.
    if write::preview_step() {
        request_world_tick();
    }
}

/// The tool's ghost of that edit, drawn by the scene.
fn ghost_tick(_: &mut TickContext) {
    use crate::domain::control::edit::ghost::ghost;
    if ghost_set(draw_list(), &ghost()) {
        request_world_tick();
    }
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_camera());
}

/// Each pyramid layer's tiles, from the live eye.
fn demand_tick(ctx: &mut TickContext) {
    // The ground's clip levels where their centers now stand, before what
    // arrives fills them.
    let at = |q: Q3| [q3::to_real(i64::from(q[0])), q3::to_real(i64::from(q[1]))];
    let pinned = clipmap::step(
        [
            pose::eye().map(|e| [e[0], e[1]]),
            controller::pivot().map(at),
            destination::pending().map(at),
        ],
        api::device_generation(),
    );
    if pinned {
        TerrainTile::demand().on_moved();
    }
    demand::admit_arrived(&ctx.pass);
    changes_step();
    TerrainTile::demand().step(destination::tile(), clipmap::pinned_tiles());
    // Nothing is fetched while places are hidden.
    match store::settings_store().place {
        true => PlaceTile::demand().step(None, &[]),
        false => PlaceTile::demand().idle(),
    }
    Earth::demand().step(None, &[]);
    DivisionTile::demand().step(division::line(controller::pivot()), &[]);
    demand::object_step();
}

/// The scene's turn, handed what control decided: the camera's pivot and,
/// third person, its heading, which the world gizmo is drawn about; the
/// edit's preview, which decor leaves out; and the marker's draft.
fn scene_tick(ctx: &mut TickContext) {
    let third_person = controller::active() == Active::Orbit;
    visible_step(
        ctx,
        controller::pivot(),
        third_person.then(camera::azimuth),
        &write::preview().edits,
        write::draft(),
    );
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step();
}

/// The camera's last writer, then the view composed where it stands and the
/// observation let go where the ground under the eye moved: after every
/// writer of the pose (the door's events, the turn's effects, the
/// controllers), before the phases that observe it. The pointer is picked
/// again from there, so what it is over follows a view moved under it.
fn camera_tick(ctx: &mut TickContext) {
    controller::camera_step(ctx);
    let moved = pose::view_compose(controller::pose());
    observation::turn();
    if let Some([x, y]) = rehover_at(moved) {
        pointer::hover(x, y);
    }
}

fn environment_tick(_: &mut TickContext) {
    environment_step();
}

/// Registers the tick's steps and spawns the tasks. Called from outside a
/// frame, since the frame task waits for the runtime's first frame.
pub fn boot() {
    on_tick(
        Phase::interaction,
        controller::controller_step,
        StepName::controller as u32,
    );
    on_tick(
        Phase::interaction,
        camera_input_tick,
        StepName::input as u32,
    );
    on_tick(Phase::interaction, camera_tick, StepName::camera as u32);
    on_tick(Phase::interaction, edit_tick, StepName::edit as u32);
    on_tick(Phase::interaction, ghost_tick, StepName::ghost as u32);
    // After the view has moved, so the environment sees this tick's camera.
    on_tick(
        Phase::interaction,
        environment_tick,
        StepName::environment as u32,
    );
    on_tick(
        Phase::interaction,
        companion_tick,
        StepName::companion as u32,
    );
    on_tick(Phase::visible, demand_tick, StepName::demand as u32);
    on_tick(Phase::visible, scene_tick, StepName::scene as u32);
    world_tasks();
}

/// No more than the executor's pool (door/executor.rs `TASKS`); a refusal means it holds fewer, and aborts.
fn world_tasks() {
    use xpute_core::sched::task::spawn;
    let spawned = spawn(frame_task(), 0) && spawn(fetch::fetch_task(), 0);
    if !spawned {
        std::process::abort();
    }
}

/// Each effect's time in the last turn, in EFFECT_NAMES' order.
static EFFECTS: Global<[f64; spec::EFFECT_NAMES]> = Global::new();

pub fn effect_times() -> [f64; spec::EFFECT_NAMES] {
    *EFFECTS.get(|| [0.0; spec::EFFECT_NAMES])
}

/// Effects are not budgeted, so nothing in them may be real work. Each is
/// timed only while the profiler records, which alone reads the times.
macro_rules! effect {
    ($times:ident, $name:ident, $step:expr) => {{
        match $times.as_mut() {
            Some(times) => {
                let at = now();
                $step;
                times[EffectName::$name as usize] += now() - at;
            }
            None => $step,
        }
    }};
}

/// Returns how long it took.
pub fn world_turn() -> f64 {
    let at = now();
    let mut times = perf_recording().then_some([0.0; spec::EFFECT_NAMES]);
    // First, so the turn they are read in is the first to see them.
    effect!(times, settings, {
        if store::settings_load_step() {
            perf_follow(store::settings_store().perf);
        }
    });
    effect!(times, navigation, off_center_step());
    effect!(times, bootstrap, map_bootstrap_step());
    effect!(times, orient, orient_watchdog_step());
    effect!(times, follow, follow_mode_step());
    effect!(times, dst, {
        // What the walker found under an unknown Z, let go where it moved.
        support::support_step();
        destination::dst_step();
    });
    effect!(times, decor, decor_models_step());
    effect!(times, objects, repository::step());
    // device_coord is not gated like pivot: the location indicator keeps its last coordinate across a transient none.
    effect!(times, location, location_step(off_center().device_coord));
    effect!(times, settings, store::settings_keep_step());
    *EFFECTS.get(|| [0.0; spec::EFFECT_NAMES]) = times.unwrap_or_default();
    now() - at
}

/// Longest gap still stepped as motion; past it (a hidden tab, a stall) the
/// world holds still rather than leaping.
const MAX_STEP_S: f64 = 0.1;

pub struct TurnGrant {
    pub delta: f64,
    /// The display's refresh, and the interval the runtime paces turns at, a
    /// whole number of refreshes: the frame's budget (the pacing the client
    /// boots xpute's runtime with, @xpute/runtime sched/frame_pacing.ts `FrameGrant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
    pub credits: u32,
}

static FRAME: Global<Option<TurnGrant>> = Global::new();
static FRAMES: Global<Notify> = Global::new();
static WORLD_DRAWN: Global<bool> = Global::new();

fn frames() -> &'static mut Notify {
    FRAMES.get(Notify::new)
}

/// A frame turn granted: one still waiting for the GPU takes this one's place, its time added.
pub fn on_world_frame_asked(grant: TurnGrant) {
    let frame = FRAME.get(|| None);
    let delta = frame.take().map_or(0.0, |was| was.delta);
    *frame = Some(TurnGrant {
        delta: delta + grant.delta,
        ..grant
    });
    frames().notify();
}

/// A turn carrying a frame polls the tasks whatever its deadline, so a turn past its deadline still draws.
pub fn world_frame_waiting() -> bool {
    FRAME.get(|| None).is_some()
}

pub fn take_world_drawn() -> bool {
    core::mem::take(WORLD_DRAWN.get(|| false))
}

/// One task, because the frame's parts have an order and the executor's
/// order among tasks is never relied on.
async fn frame_task() {
    loop {
        notified(frames).await;
        // A frame the GPU cannot take without blocking (xpute_gpu::admit) is not run: its grant
        // waits for the next, so the turn hands the renderer nothing it would have to put off.
        if api::device().is_some() && !boot::admit_frame()() {
            request_world_tick();
            continue;
        }
        let Some(grant) = FRAME.get(|| None).take() else {
            continue;
        };
        let edge = world_edge();
        let paced_ms = grant.paced_ms;
        world_frame(grant, edge.deadline_ms(), edge.edge_at());
        edge::tasks_after_frame(paced_ms);
        clipmap::fill_share();
        WORLD_DRAWN.set(true);
    }
}

fn world_frame(grant: TurnGrant, deadline_ms: f64, edge_at: f64) -> TickReport {
    let TurnGrant {
        delta,
        refresh_ms,
        paced_ms,
        interactive,
        credits,
    } = grant;
    let t0 = now();
    set_credits(credits);
    text_rise();
    let effects_ms = world_turn();
    // Step by at most MAX_STEP_S, or a pause makes a character cross a wall or
    // a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = run_tick(step, deadline_ms, edge_at);
    let after_tick = now();
    // The turn's one pump, after every producer has spoken: the effects, the
    // tick's tile streams (`demand_tick`), and the assets asked for by either.
    request::declare(Stream::asset, loader::load_wants());
    pump();
    let drawn = stage::stage_render(
        Pace {
            refresh_ms,
            paced_ms,
            gap_ms: delta * 1000.0,
            moving: interactive,
        },
        &write::preview().edits,
    );
    set_renderer_frame(&drawn);
    if let Some(interval_ms) = drawn.interval_ms {
        cmd::shell::turn_pace(&cmd::TurnPace { interval_ms });
    }
    let done = now();
    perf_frame(FrameTurn {
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
/// (domain/query/changes.rs). Taken after arrivals are admitted, before the
/// layers observe.
fn changes_step() {
    static MOVED: Global<[Vec<Quadrant>; 3]> = Global::new();
    let [places, earth, divisions] = MOVED.get(Default::default);
    let shared = changes::take();
    gpu::note_edited(changes::written_boxes());
    PlaceTile::tile_cache().take_moved(places);
    Earth::tile_cache().take_moved(earth);
    DivisionTile::tile_cache().take_moved(divisions);
    if shared {
        TerrainTile::demand().on_moved();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().on_moved();
        marker::on_moved(places);
    }
    if shared || !earth.is_empty() {
        Earth::demand().on_moved();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().on_moved();
    }
}
