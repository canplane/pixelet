// pxl-core/turn.rs

//! The order of a world turn is written here and nowhere else. A frame runs
//! as one task: first the effects, then the tick's phases under one shared
//! budget, then the draw. Every other task waits on its own input and runs
//! apart from the frame.

#[path = "turn/step.spec.rs"]
pub mod spec;

use crate::domain::layer::Base as _;
use pxl_spec::space::coord::q3::{self, Q3};
use spec::{EffectName, StepName};

use crate::domain::control::input::{camera_input_step, rehover_at};
use crate::domain::control::navigation::bootstrap::map_bootstrap_step;
use crate::domain::control::navigation::follow_mode::follow_mode_step;
use crate::domain::control::navigation::off_center::{off_center, off_center_step};
use crate::domain::control::sensor::device_orientation::orient_watchdog_step;
use crate::sys::io::param::CREDITS;
use crate::sys::io::request::{pump, set_credits};
use crate::sys::io::text::text_drop_unread;
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
use crate::domain::layer::earth::EarthTile;
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
use crate::domain::scene::rate;
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
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::sched::tick::{TickContext, TickReport};
use crate::sys::sched::display;
use crate::sys::sched::{self, executor, invalidate};
use xpute_gpu::abi::vsync;
use pxl_render::gpu::api;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_core::clock::now;
use xpute_core::global::Global;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::sys::sched::param::MAX_STEP_S;
use crate::sys::sched::phase_spec::Phase;

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
    if write::render_edit_step() {
        invalidate();
    }
}

fn ghost_tick(_: &mut TickContext) {
    use crate::domain::control::edit::ghost::ghost;
    if ghost_set(draw_list(), &ghost()) {
        invalidate();
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
    demand::admit_arrived();
    changes_step();
    TerrainTile::demand().step(destination::tile(), clipmap::pinned_tiles());
    // No place tile is fetched while places are hidden.
    match store::settings().place {
        true => PlaceTile::demand().step(None, &[]),
        false => PlaceTile::demand().idle(),
    }
    EarthTile::demand().step(None, &[]);
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
        &write::render_edit(),
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

/// Runs once, outside any frame. The frame task it spawns waits until a
/// frame is asked for.
pub fn boot() {
    world_tasks();
}

type Step = (StepName, fn(&mut TickContext));

/// The tick's steps, phase by phase, in the order they run.
const STEPS: [(Phase, &[Step]); 2] = [
    (
        Phase::interaction,
        &[
            (StepName::controller, controller::controller_step),
            (StepName::input, camera_input_tick),
            (StepName::camera, camera_tick),
            (StepName::edit, edit_tick),
            (StepName::ghost, ghost_tick),
            // The environment runs after the camera, so it sees this tick's
            // camera.
            (StepName::environment, environment_tick),
            (StepName::companion, companion_tick),
        ],
    ),
    (
        Phase::visible,
        &[
            (StepName::demand, demand_tick),
            (StepName::scene, scene_tick),
        ],
    ),
];

fn world_tick(delta: f64) -> TickReport {
    let start = now();
    let mut ctx = TickContext::new(delta);
    for (phase, steps) in STEPS {
        ctx.phase(phase, |ctx| {
            for &(name, step) in steps {
                timed(ctx, phase, name, step);
            }
        });
    }
    ctx.report(start)
}

/// A step's time, summed while the profiler records.
#[derive(Clone, Copy, Debug)]
pub struct StepStat {
    pub phase: Phase,
    pub ms: f64,
    pub cnt: u32,
    pub max_ms: f64,
}

const STAT_NONE: StepStat = StepStat {
    phase: Phase::ALL[0],
    ms: 0.0,
    cnt: 0,
    max_ms: 0.0,
};

/// Each step's time by its name, since the profiler last began to record.
static STEP_STATS: Global<[StepStat; spec::STEP_NAMES]> = Global::new();
/// Whether the profiler records, and the steps are timed.
static STEP_TIMING: Global<bool> = Global::new();

fn step_stats_mut() -> &'static mut [StepStat; spec::STEP_NAMES] {
    STEP_STATS.get(|| [STAT_NONE; spec::STEP_NAMES])
}

/// Runs the step `name` of `phase`, timed while the profiler records.
fn timed(ctx: &mut TickContext, phase: Phase, name: StepName, step: fn(&mut TickContext)) {
    if !*STEP_TIMING.get(|| false) {
        return step(ctx);
    }
    let at = now();
    step(ctx);
    let ms = now() - at;
    let s = &mut step_stats_mut()[name as usize];
    (s.phase, s.ms, s.cnt, s.max_ms) = (phase, s.ms + ms, s.cnt + 1, s.max_ms.max(ms));
}

/// Starts summing each step's time from zero, or stops summing it; what was
/// summed stays to be read.
pub fn set_step_timing(on: bool) {
    if on {
        *step_stats_mut() = [STAT_NONE; spec::STEP_NAMES];
    }
    STEP_TIMING.set(on);
}

/// Each step that ran while the profiler recorded, by its name.
pub fn step_stats() -> impl Iterator<Item = (StepName, StepStat)> {
    let stats = *step_stats_mut();
    StepName::ALL
        .into_iter()
        .map(move |name| (name, stats[name as usize]))
        .filter(|(_, s)| s.cnt > 0)
}

/// Spawns the world's frame task, one of embassy's static tasks, which runs
/// for the core's life.
fn world_tasks() {
    executor::spawner().spawn(frame_task().or_bug(Errno::ENOSPC));
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

static REFUSED: Global<Refused> = Global::new();
/// When the last frame ran.
static LAST: Global<Option<f64>> = Global::new();

/// The frames that did not run since boot, by why (`frame_task`): the GPU
/// admitted none, or the runtime still held the stream.
#[derive(Clone, Copy, Default)]
pub struct Refused {
    pub admit: f64,
    pub stream: f64,
}

pub fn refused_frames() -> Refused {
    *REFUSED.get(Refused::default)
}

/// The frame runs as one task because its parts must run in order, and
/// nothing may rely on the order in which the executor runs tasks. It waits
/// until a frame is asked for, then for vsyncs, each measuring the display
/// (sys/sched/display.rs), until the frame's target interval (rate.rs) has
/// passed since the last frame, within half a display interval, and runs the
/// world's frame.
#[embassy_executor::task]
async fn frame_task() {
    loop {
        if !sched::is_invalid() {
            display::idle();
        }
        sched::invalidated().await;
        loop {
            vsync::next().await.or_bug(Errno::EPROTO);
            let at = now();
            display::answered(at);
            let display_ms = display::interval_ms();
            let target_ms = f64::from(rate::intervals()) * display_ms;
            if LAST.get(|| None).is_none_or(|last| at - last >= target_ms - display_ms / 2.0) {
                break;
            }
        }
        // A frame whose stream the runtime still holds, or that the GPU
        // cannot take without blocking (xpute_gpu::admit), does not run, is
        // counted by why, and asks for the next, so the renderer is never
        // handed a frame it would have to put off.
        let refused = REFUSED.get(Refused::default);
        let why = if !api::stream_free() {
            Some(&mut refused.stream)
        } else if api::device().is_some() && !crate::admit_frame() {
            Some(&mut refused.admit)
        } else {
            None
        };
        if let Some(cnt) = why {
            *cnt += 1.0;
            invalidate();
            continue;
        }
        world_frame();
        clipmap::fill_share();
    }
}

/// Runs the world's frame in its order: the effects, the tick, the requests'
/// pump and the stage. It steps the world by the time since the last frame,
/// and the frame is moving where it asked for the next. The frame rate then
/// judges the frame by its gap and its cost, the GPU's busy time where a read
/// timed it and the frame's own.
fn world_frame() -> TickReport {
    let t0 = now();
    let gap_ms = LAST.get(|| None).map_or(0.0, |at| t0 - at);
    let delta = gap_ms / 1000.0;
    set_credits(CREDITS);
    text_drop_unread();
    let effects_ms = world_turn();
    // A step longer than MAX_STEP_S would carry a character through a wall
    // or make a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = world_tick(step);
    let after_tick = now();
    // Requests are pumped once a frame, after every producer has declared its
    // wants: the effects, the tick's tile streams (`demand_tick`), and the
    // assets either of them asked for.
    request::declare(Stream::asset, loader::load_wants());
    pump();
    let moving = sched::is_invalid();
    let display_ms = display::interval_ms();
    let frame = stage::stage_render(
        Pace {
            gap_ms,
            display_ms,
            moving,
        },
        &write::render_edit(),
    );
    let end = now();
    let gpu_ms = frame.stat.as_ref().and_then(|s| s.gpu).map_or(0.0, |g| g.busy_ms);
    rate::judge(gap_ms, gpu_ms.max(end - t0), display_ms);
    set_renderer_frame(&frame);
    perf_frame(FrameTurn {
        tick: report,
        refresh_ms: display_ms,
        paced_ms: gap_ms,
        settled: !moving,
        effects_ms,
        stage_ms: end - after_tick,
    });
    LAST.set(Some(t0));
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
    gpu::note_damage(change::damage());
    PlaceTile::tile_cache().take_changed(places);
    EarthTile::tile_cache().take_changed(earth);
    DivisionTile::tile_cache().take_changed(divisions);
    if shared {
        TerrainTile::demand().on_changed();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().on_changed();
        marker::on_changed(places);
    }
    if shared || !earth.is_empty() {
        EarthTile::demand().on_changed();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().on_changed();
    }
}
