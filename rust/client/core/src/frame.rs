// pxl-core/frame.rs

//! The order of a frame is written here and nowhere else. A frame runs
//! as one task: first the effects, then the tick's phases under one shared
//! budget, then the draw. Every other task waits on its own input and runs
//! apart from the frame.

use crate::domain::layer::Base as _;
use crate::spec::step::{EffectName, InvalidationReason, STEP_PHASE, StepName};
use pxl_spec::space::coord::q3::{self, Q3};

use crate::domain::control::input::{camera_input_step, rehover_at};
use crate::domain::layer::terrain::tile::IndexedTerrainTile;
use crate::sys::io::request::pump;
use crate::sys::io::text::text_drop_unread;
use pxl_spec::bake::tile::place::PlaceTile;

use crate::domain::control::camera::controller;
use crate::domain::control::edit::write;
use crate::domain::control::input;
use crate::domain::control::navigation;
use crate::domain::control::navigation::location::user_location;
use crate::domain::eye;
use crate::domain::layer::division;
use crate::domain::layer::imagery::ImageryTile;
use crate::domain::layer::object::repository;
use crate::domain::layer::terrain::clipmap;
use crate::domain::query::change;
use crate::domain::scene::character::companion::{Camera, companion_step};
use crate::domain::scene::decor::asset::decor_models_step;
use crate::domain::scene::decor::gpu;
use crate::domain::scene::environment::environment_step;
use crate::domain::scene::indicator::ghost::ghost_set;
use crate::domain::scene::indicator::location::location_step;
use crate::domain::scene::marker;
use crate::domain::scene::model::draw_list;
use crate::domain::scene::rate::{self, Motion, Pace};
use crate::domain::scene::stage;
use crate::domain::scene::stage::scene_step;
use crate::domain::setting;
use crate::domain::setting::CameraMode;
use crate::domain::view::demand;
use crate::domain::view::demand::DemandedBase as _;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::perf::frame::EFFECT_CNT;
use crate::perf::record::{perf_record_fill, perf_recording, timed};
use crate::perf::report::{CpuSample, perf_frame, set_renderer_frame};
use crate::perf::window::perf_follow;
use crate::pointer;
use crate::sys::io::demand::Stream;
use crate::sys::io::loader;
use crate::sys::io::request;
use crate::sys::sched::display;
use crate::sys::sched::tick::{TickContext, TickReport};
use crate::sys::sched::{self, executor, invalidate};
use pxl_global::Global;
use pxl_gpu_guest::abi::vsync;
use pxl_render::gpu::api;
use pxl_spec::bake::tile::division::DivisionTile;
use pxl_spec::space::coord::zoom::LEAF_ZOOM;
use pxl_spec::space::layer::lattice::{GROUND, Grid};
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_guest::clock::{self, now};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::spec::phase::Phase;
use crate::sys::sched::param::MAX_STEP_S;

/// The camera as the companion's walk reads it. It is None in first person,
/// where the eye stands over the pivot and the person is the one looking.
fn companion_camera() -> Option<Camera> {
    if controller::mode() != CameraMode::Orbit {
        return None;
    }
    // The pivot the camera is easing toward, not the one its easing has
    // reached so far.
    let goal = controller::pivot_to()?;
    eye::at()?;
    let at = coord::to_world(&goal);
    Some(Camera {
        pivot: [at[0], at[1]],
        floor: controller::floor_to()?,
        azimuth: controller::facing()?.1,
        dragging: input::dragging(),
        steered: input::steering(),
        tracking: navigation::is_tracking(),
    })
}

/// Works out the edit the pointer would make on what it is over. It runs
/// after `camera_tick` has placed the camera, so the scene draws the edit
/// this frame and not the next.
fn edit_tick(_: &mut TickContext) {
    // A changed preview changes the world as drawn, both where the old
    // preview was and where the new one is.
    if write::render_edit_step() {
        invalidate(InvalidationReason::Scene);
    }
}

fn ghost_tick(_: &mut TickContext) {
    use crate::domain::control::edit::ghost::ghost;
    if ghost_set(draw_list(), &ghost()) {
        invalidate(InvalidationReason::Scene);
    }
}

/// The indicator is handed the user's location as the device reports it,
/// None included. It keeps showing the last while the device briefly reports
/// none.
fn location_tick(_: &mut TickContext) {
    location_step(user_location());
}

fn companion_tick(ctx: &mut TickContext) {
    companion_step(ctx, companion_camera());
}

/// Asks every tile pyramid for the tiles the eye needs now.
fn demand_tick(_: &mut TickContext) {
    // The ground's clip levels move to their new centers before arrived
    // tiles are admitted, so each arrival fills a level where it now stands.
    let at = |q: Q3| [q3::to_real(i64::from(q[0])), q3::to_real(i64::from(q[1]))];
    // The place being made a point, whose leaves are asked for.
    let to = navigation::destination().map(|at| (at, GROUND.cell_of(LEAF_ZOOM, &at).tile()));
    let pinned = clipmap::step(
        [
            eye::at().map(|e| [e[0], e[1]]),
            controller::pivot().map(at),
            to.map(|(p, _)| at(p)),
        ],
        api::device_generation(),
    );
    if pinned {
        IndexedTerrainTile::demand().on_changed();
    }
    demand::admit_arrived();
    changes_step();
    IndexedTerrainTile::demand().step(to.map(|(_, t)| t), clipmap::pinned_tiles());
    // No place tile is fetched while places are hidden.
    match setting::settings().place {
        true => PlaceTile::demand().step(None, &[]),
        false => PlaceTile::demand().idle(),
    }
    ImageryTile::demand().step(None, &[]);
    DivisionTile::demand().step(division::tile(controller::pivot()), &[]);
    demand::object_step();
}

/// Runs the scene's frame with what control decided. The world gizmo is drawn
/// about the camera's pivot, and for the orbit also about the view's
/// heading.
/// Decor leaves out what the edit's preview takes away.
fn scene_tick(ctx: &mut TickContext) {
    let orbits = controller::mode() == CameraMode::Orbit;
    scene_step(
        ctx,
        controller::pivot(),
        orbits.then(pose::azimuth).flatten(),
        &write::render_edit(),
        write::draft(),
    );
}

fn camera_input_tick(_: &mut TickContext) {
    camera_input_step();
}

/// Writes the camera for the last time this frame and has the view take its
/// pose where it differs by what can be told apart (`view_compose`). It must
/// run after every writer of the pose (the link's
/// events, the frame's effects, the hand's input) and before every phase that
/// observes the view. The pointer is picked again from the new view, so what
/// it is over stays right when the view moves under a still pointer.
fn camera_tick(ctx: &mut TickContext) {
    controller::camera_step(ctx);
    let moved = pose::view_compose(controller::pose());
    observation::forget_touched();
    if let Some([x, y]) = rehover_at(moved) {
        pointer::hover(x, y);
    }
}

fn environment_tick(_: &mut TickContext) {
    environment_step();
}

/// Runs once, outside any frame. The frame task it spawns waits until a
/// frame is asked for.
pub(crate) fn boot() {
    start();
}

type Step = (StepName, fn(&mut TickContext));

/// The tick's steps, in the order they run, each in its phase (step.json5
/// `STEP_PHASE`).
const STEPS: [Step; StepName::ALL.len()] = [
    (StepName::Input, camera_input_tick),
    (StepName::Camera, camera_tick),
    (StepName::Edit, edit_tick),
    (StepName::Ghost, ghost_tick),
    (StepName::Location, location_tick),
    // The environment runs after the camera, so it sees this tick's camera.
    (StepName::Environment, environment_tick),
    (StepName::Companion, companion_tick),
    (StepName::Demand, demand_tick),
    (StepName::Scene, scene_tick),
];

fn tick(delta: f64) -> TickReport {
    let start = now();
    let mut ctx = TickContext::new(delta);
    for phase in Phase::ALL {
        ctx.phase(phase, |ctx| {
            for (name, step) in STEPS {
                if STEP_PHASE[name as usize] == phase {
                    timed(ctx, name, step);
                }
            }
        });
    }
    ctx.report(start)
}

/// Spawns the world's frame task, one of embassy's static tasks, which runs
/// for the core's life.
fn start() {
    executor::spawner().spawn(frame_task().or_bug(Errno::ENOSPC));
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

/// Runs the frame's effects, and returns how long each took, indexed by
/// `EffectName`, in milliseconds.
pub(crate) fn effects() -> [f64; EFFECT_CNT] {
    let mut times = perf_recording().then_some([0.0; EFFECT_CNT]);
    // Settings load first, so the frame that reads new settings is also the
    // first to act on them.
    effect!(times, Setting, {
        if setting::settings_load_step() {
            perf_follow(setting::settings().perf);
        }
    });
    effect!(times, Navigation, {
        controller::forget_touched();
        navigation::navigation_step(input::gesturing());
    });
    effect!(times, Decor, decor_models_step());
    effect!(times, Object, repository::step());
    effect!(times, Setting, setting::settings_keep_step());
    times.unwrap_or_default()
}

static REFUSED: Global<Refused> = Global::new();
/// When the last drawn frame started, its vsync's mark, and whether it moved.
static LAST: Global<Option<(f64, bool)>> = Global::new();

/// The frames that did not run since boot, by why (`frame_task`): the GPU
/// admitted none, or the host still held the stream.
#[derive(Clone, Copy, Default)]
pub(crate) struct Refused {
    pub admit: f64,
    pub stream: f64,
}

pub(crate) fn refused_frames() -> Refused {
    *REFUSED.get(Refused::default)
}

/// The frame runs as one task because its parts must run in order, and
/// nothing may rely on the order in which the executor runs tasks. It waits
/// until a frame is asked for, then for the first vsync from when the frame
/// is due by the target interval since the last frame's start, run or
/// refused (rate.rs `due_at`), which answers its frame's start and the gap
/// the display is measured by (sys/sched/display.rs). The budget is told the
/// target the frame is asked on (sys/sched.rs `set_pace_ms`) and its start
/// (`begin_frame`), and the world's frame runs
/// and ends its entry's budget (`end_frame`), so the entry ends and the
/// frame's stream leaves for the GPU (link.rs `end`). The task then asks for
/// the next frame's vsync where one is asked for, in that entry, so no other
/// task's work stands before the ask. The ground's fill, which prepares later
/// frames, is a task of its own, started where bricks are due (`fill`); its
/// one slot is taken while a fill is out, and a frame then starts none.
#[embassy_executor::task]
async fn frame_task() {
    loop {
        sched::invalidated().await;
        let (display_ms, target_ms) = rate::pace_ms();
        let due_at = rate::due_at(sched::frame_at(), (display_ms, target_ms));
        sched::set_pace_ms((display_ms, target_ms));
        let frame = vsync::next(clock::mark(due_at)).await.or_bug(Errno::EPROTO);
        display::answered(clock::ms(frame.gap));
        let at = clock::ms(frame.at);
        sched::begin_frame(at);
        // A frame whose stream the host still holds, or that the GPU cannot
        // take without blocking (`pxl_gpu_guest::admit::frame`), does not
        // run, is counted by why, and asks for the next, so the renderer is
        // never handed a frame it would have to put off.
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
            invalidate(InvalidationReason::Pending);
            continue;
        }
        run(at, display_ms, target_ms);
        sched::end_frame();
        if clipmap::fill_due()
            && let Ok(task) = fill()
        {
            executor::spawner().spawn(task);
        }
    }
}

/// Fills the ground's due bricks by one entry's share, apart from the frame
/// as every task is: it leaves the frame's entry, which the frame's end has
/// spent, and fills in the entry that wakes it, on that entry's budget
/// (clipmap.rs `fill_bricks`), timed while the profiler records into the
/// record of the frame it follows.
#[embassy_executor::task]
async fn fill() {
    sched::yield_if_spent().await;
    let at = perf_recording().then(now);
    let fill_brick_cnt = clipmap::fill_bricks();
    if let Some(at) = at {
        perf_record_fill(now() - at, fill_brick_cnt);
    }
}

/// Runs the world's frame, which started at `at`, paced on a display interval
/// of `display_ms` to a target of `target_ms`, in its order: the effects, the
/// tick, the requests' pump and the stage. It steps the world by the gap since
/// the last drawn frame's start, and the frame is moving where it asked for
/// the next. The frame rate then judges a moving frame that does not start
/// motion by its gap and its cost, the GPU's busy time where a read timed it
/// and the frame's own.
fn run(at: f64, display_ms: f64, target_ms: f64) -> TickReport {
    let t0 = now();
    let last = *LAST.get(|| None);
    let gap_ms = last.map_or(0.0, |(last_at, _)| at - last_at);
    let delta = gap_ms / 1000.0;
    text_drop_unread();
    let effects = effects();
    // A step longer than `MAX_STEP_S` would carry a character through a
    // wall or make a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = tick(step);
    // The pump and the stage's parts are timed only while the profiler
    // records, as an effect is.
    let recording = perf_recording();
    let after_tick = recording.then(now);
    // Requests are pumped once a frame, after every producer has declared its
    // wants: the effects, the tick's tile streams (`demand_tick`), and the
    // assets either of them asked for.
    request::declare(Stream::Asset, loader::load_wants());
    pump();
    let pump_ms = after_tick.map_or(0.0, |at| now() - at);
    let moving = sched::is_invalid();
    let pace = Pace {
        gap_ms,
        display_ms,
        motion: match (moving, last.is_some_and(|(_, moved)| moved)) {
            (false, _) => Motion::Still,
            (true, false) => Motion::Start,
            (true, true) => Motion::Moving,
        },
    };
    let frame = stage::stage_render(pace, &write::render_edit(), recording);
    let end = now();
    let gpu_ms = frame
        .stat
        .as_ref()
        .and_then(|s| s.gpu)
        .map_or(0.0, |g| g.busy_ms);
    rate::judge(pace, gpu_ms.max(end - t0));
    set_renderer_frame(&frame);
    perf_frame(CpuSample {
        tick: report,
        gap_ms,
        refresh_ms: display_ms,
        paced_ms: target_ms,
        settled: !moving,
        effects,
        pump_ms,
        stage_parts: frame.parts,
        invalidations: sched::take_invalidations(),
    });
    LAST.set(Some((at, moving)));
    report
}

/// Tells each reader what changed this frame: the tiles whose answer changed
/// and the boxes where objects were written. A tile is reported by its
/// identity, which fixes its contents. The query holds the ground's changes
/// (domain/query/change.rs). This runs after arrivals are admitted and
/// before the layers observe.
fn changes_step() {
    static CHANGED: Global<[Vec<Quadrant>; 3]> = Global::new();
    let [places, imagery, divisions] = CHANGED.get(Default::default);
    let shared = change::take();
    gpu::note_damage(change::damage());
    PlaceTile::tile_cache().take_changed(places);
    ImageryTile::tile_cache().take_changed(imagery);
    DivisionTile::tile_cache().take_changed(divisions);
    if shared {
        IndexedTerrainTile::demand().on_changed();
    }
    if shared || !places.is_empty() {
        PlaceTile::demand().on_changed();
        marker::on_changed(places);
    }
    if shared || !imagery.is_empty() {
        ImageryTile::demand().on_changed();
    }
    if !divisions.is_empty() {
        DivisionTile::demand().on_changed();
    }
}
