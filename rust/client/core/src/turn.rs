// pxl-core/turn.rs

//! The order of a world turn is written here and nowhere else. A frame runs
//! as one task: first the effects, then the tick's phases under one shared
//! budget, then the draw. A turn that draws no frame takes in the world's
//! content alone. Every other task waits on its own input and runs apart
//! from the frame.

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
use crate::domain::query::change;
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
use crate::domain::scene::stage::scene_step;
use crate::domain::setting::store;
use crate::domain::view::demand;
use crate::domain::view::demand::ViewLayer as _;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
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
use crate::sys::sched::tick::{tick, TickContext};
use pxl_render::gpu::api;
use pxl_render::gpu::span_spec::FrameKind;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_core::sched::pass::Pass;
use xpute_core::sched::tick::TickReport;

use crate::sys::sched::param::{HARD_SLACK_MS, MAX_STEP_S};
use crate::sys::sched::phase_spec::Phase;
use crate::sys::sched::queue::Queue;

/// The camera as the companion's walk reads it. It is None in first person,
/// where the eye stands at the pivot and the person is the one looking.
fn companion_camera() -> Option<Camera> {
    if controller::active() != Active::Orbit {
        return None;
    }
    // The pivot the camera is easing toward, not the one its easing has
    // reached so far.
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

/// Works out the edit the pointer would make on what it is over. It runs
/// after `camera_tick` has placed the camera, so the scene draws the edit
/// this turn and not the next.
fn edit_tick(_: &mut TickContext) {
    // A changed preview changes the world as drawn, both where the old
    // preview was and where the new one is.
    if write::preview_step() {
        request_world_tick();
    }
}

fn ghost_tick(_: &mut TickContext) {
    use crate::domain::control::edit::ghost::ghost;
    if ghost_set(draw_list(), &ghost()) {
        request_world_tick();
    }
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_camera());
}

/// Asks every tile pyramid for the tiles the eye needs now.
fn demand_tick(ctx: &mut TickContext) {
    // The ground's clip levels move to their new centers before arrived
    // tiles are admitted, so each arrival fills a level where it now stands.
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
        TerrainTile::demand().on_changed();
    }
    demand::admit_arrived(&ctx.pass);
    changes_step();
    TerrainTile::demand().step(destination::tile(), clipmap::pinned_tiles());
    // No place tile is fetched while places are hidden.
    match store::settings().place {
        true => PlaceTile::demand().step(None, &[]),
        false => PlaceTile::demand().idle(),
    }
    Earth::demand().step(None, &[]);
    DivisionTile::demand().step(division::line(controller::pivot()), &[]);
    demand::object_step();
}

/// Runs the scene's turn with what control decided. The world gizmo is drawn
/// about the camera's pivot, and in third person also about its heading.
/// Decor leaves out what the edit's preview takes away.
fn scene_tick(ctx: &mut TickContext) {
    let third_person = controller::active() == Active::Orbit;
    scene_step(
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

/// Writes the camera for the last time this turn and composes the view where
/// it now stands. It must run after every writer of the pose (the door's
/// events, the turn's effects, the controllers) and before every phase that
/// observes the view. The pointer is picked again from the new view, so what
/// it is over stays right when the view moves under a still pointer.
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

/// Runs once, outside any frame. The frame task it spawns sleeps until the
/// runtime grants the first frame.
pub fn boot() {
    tick().on_tick(
        Phase::interaction as u8,
        controller::controller_step,
        StepName::controller as u32,
    );
    tick().on_tick(
        Phase::interaction as u8,
        camera_input_tick,
        StepName::input as u32,
    );
    tick().on_tick(
        Phase::interaction as u8,
        camera_tick,
        StepName::camera as u32,
    );
    tick().on_tick(Phase::interaction as u8, edit_tick, StepName::edit as u32);
    tick().on_tick(Phase::interaction as u8, ghost_tick, StepName::ghost as u32);
    // The environment runs after the camera, so it sees this tick's camera.
    tick().on_tick(
        Phase::interaction as u8,
        environment_tick,
        StepName::environment as u32,
    );
    tick().on_tick(
        Phase::interaction as u8,
        companion_tick,
        StepName::companion as u32,
    );
    tick().on_tick(Phase::visible as u8, demand_tick, StepName::demand as u32);
    tick().on_tick(Phase::visible as u8, scene_tick, StepName::scene as u32);
    world_tasks();
}

/// The world's tasks must fit in the executor's pool (door/executor.rs
/// `TASKS`). A refused spawn means the pool is too small, and the core aborts.
fn world_tasks() {
    use xpute_core::sched::task::spawn;
    let spawned = spawn(frame_task(), 0) && spawn(fetch::fetch_task(), 0);
    if !spawned {
        std::process::abort();
    }
}

/// The milliseconds each effect took in the last turn, indexed by
/// `EffectName`.
static EFFECTS: Global<[f64; spec::EFFECT_NAMES]> = Global::new();

pub fn effect_times() -> [f64; spec::EFFECT_NAMES] {
    *EFFECTS.get(|| [0.0; spec::EFFECT_NAMES])
}

/// Effects run outside the tick's budget, so an effect must not do real
/// work. An effect is timed only while the profiler records, because the
/// profiler is the only reader of the times.
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

/// Runs the turn's effects and returns how long they took, in milliseconds.
pub fn world_turn() -> f64 {
    let at = now();
    let mut times = perf_recording().then_some([0.0; spec::EFFECT_NAMES]);
    // Settings load first, so the turn that reads new settings is also the
    // first to act on them.
    effect!(times, settings, {
        if store::settings_load_step() {
            perf_follow(store::settings().perf);
        }
    });
    effect!(times, navigation, off_center_step());
    effect!(times, bootstrap, map_bootstrap_step());
    effect!(times, orient, orient_watchdog_step());
    effect!(times, follow, follow_mode_step());
    effect!(times, dst, {
        // The floors the walker found under a position of unknown height
        // are dropped where the ground changed under them.
        support::support_step();
        destination::dst_step();
    });
    effect!(times, decor, decor_models_step());
    effect!(times, objects, repository::step());
    // The indicator is handed the device's coordinate as reported, None
    // included. It keeps showing its last coordinate while the device briefly
    // reports none.
    effect!(times, location, location_step(off_center().device_coord));
    effect!(times, settings, store::settings_keep_step());
    *EFFECTS.get(|| [0.0; spec::EFFECT_NAMES]) = times.unwrap_or_default();
    now() - at
}

pub struct TurnGrant {
    pub delta: f64,
    /// The display's refresh interval, and the interval the runtime paces
    /// turns at (`paced_ms`), both in milliseconds. The paced interval is a
    /// whole number of refreshes and is the frame's budget. The client sets
    /// this pacing when it boots xpute's runtime (@xpute/runtime
    /// sched/frame_pacing.ts `FrameGrant`).
    pub refresh_ms: f64,
    pub paced_ms: f64,
    pub interactive: bool,
    pub credits: u32,
}

static FRAME: Global<Option<TurnGrant>> = Global::new();
static FRAMES: Global<Notify> = Global::new();
static NEW_FRAME: Global<bool> = Global::new();
static QUEUE: Global<Queue> = Global::new();

fn frames() -> &'static mut Notify {
    FRAMES.get(Notify::new)
}

fn queue() -> &'static mut Queue {
    QUEUE.get(Queue::default)
}

/// Whether this turn draws the frame it was granted. It does not where the GPU cannot take the
/// frame without blocking (xpute_gpu::admit), where the runtime still holds the stream, or
/// where a later turn would still reach the GPU in time, which shows a newer input.
fn frame_due() -> bool {
    let paced_ms = FRAME.get(|| None).as_ref().map_or(0.0, |g| g.paced_ms);
    boot::admit_frame()() && api::stream_free() && queue().due(now(), api::unfinished(), paced_ms)
}

/// A turn that draws no frame still takes in what arrived, starts the reads that freed and
/// fills the bricks due, so the world's content does not wait on the GPU. It works from what
/// the last frame asked for and where its clip levels stand, which only a frame moves.
fn content_turn() {
    let edge = world_edge();
    demand::admit_arrived(&Pass::new(
        edge.deadline_ms(),
        edge.edge_at(),
        HARD_SLACK_MS,
    ));
    pump();
    clipmap::fill_share();
}

/// Takes a frame the runtime granted. A frame still waiting for the GPU is
/// replaced by this one, and its elapsed time is added to this one's.
pub fn on_world_frame_asked(grant: TurnGrant) {
    let frame = FRAME.get(|| None);
    let delta = frame.take().map_or(0.0, |was| was.delta);
    *frame = Some(TurnGrant {
        delta: delta + grant.delta,
        ..grant
    });
    frames().notify();
}

/// Whether a granted frame waits to run. A turn that carries a frame polls
/// the tasks whatever its deadline, so even a late turn draws.
pub fn world_frame_waiting() -> bool {
    FRAME.get(|| None).is_some()
}

pub fn take_new_frame() -> bool {
    core::mem::take(NEW_FRAME.get(|| false))
}

/// The frame runs as one task because its parts must run in order, and
/// nothing may rely on the order in which the executor runs tasks.
async fn frame_task() {
    loop {
        notified(frames).await;
        // A frame not due does not run. Its grant waits for the next turn, so
        // the renderer is never handed a frame it would have to put off.
        if api::device().is_some() && !frame_due() {
            content_turn();
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
        NEW_FRAME.set(true);
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
    // A step longer than MAX_STEP_S would carry a character through a wall
    // or make a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = tick().run(step, deadline_ms, edge_at);
    let after_tick = now();
    // Requests are pumped once a turn, after every producer has declared its
    // wants: the effects, the tick's tile streams (`demand_tick`), and the
    // assets either of them asked for.
    request::declare(Stream::asset, loader::load_wants());
    pump();
    let frame = stage::stage_render(
        Pace {
            refresh_ms,
            paced_ms,
            gap_ms: delta * 1000.0,
            moving: interactive,
        },
        &write::preview().edits,
    );
    set_renderer_frame(&frame);
    if let Some(interval_ms) = frame.interval_ms {
        cmd::ui::turn_pace(&cmd::TurnPace { interval_ms });
    }
    let done = now();
    if let Some(gpu) = frame.stat.and_then(|stat| stat.gpu) {
        queue().measure(gpu.busy_ms);
    }
    // A frame counted from 1 was encoded; 0 stood and drew nothing.
    if frame.kind != FrameKind::dropped && frame.stat.is_some_and(|stat| stat.frame > 0.0) {
        queue().submit(done, done - t0);
    }
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

/// Tells each reader what changed this turn: the tiles whose answer changed
/// and the boxes where objects were written. A tile is reported by its
/// identity, which fixes its contents. The query holds the ground's changes
/// (domain/query/change.rs). This runs after arrivals are admitted and
/// before the layers observe.
fn changes_step() {
    static CHANGED: Global<[Vec<Quadrant>; 3]> = Global::new();
    let [places, earth, divisions] = CHANGED.get(Default::default);
    let shared = change::take();
    gpu::note_edited(change::written_boxes());
    PlaceTile::tile_cache().take_changed(places);
    Earth::tile_cache().take_changed(earth);
    DivisionTile::tile_cache().take_changed(divisions);
    if shared {
        TerrainTile::demand().on_changed();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().on_changed();
        marker::on_changed(places);
    }
    if shared || !earth.is_empty() {
        Earth::demand().on_changed();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().on_changed();
    }
}
