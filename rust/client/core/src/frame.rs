// pxl-core/frame.rs

//! The order of a frame is written here and nowhere else. A frame runs
//! as one task: first the effects, then the tick's phases under one shared
//! budget, then the draw. Every other task waits on its own input and runs
//! apart from the frame.

use crate::domain::layer::Base as _;
use crate::spec::step::{EffectName, StepName};
use pxl_spec::space::coord::q3::{self, Q3};

use crate::domain::control::input::{camera_input_step, rehover_at};
use crate::domain::control::navigation::bootstrap::map_bootstrap_step;
use crate::domain::control::navigation::follow_mode::follow_mode_step;
use crate::domain::control::navigation::off_center::{off_center, off_center_step};
use crate::sys::io::param::CREDIT_CNT;
use crate::sys::io::request::{pump, set_credit_cnt};
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
use crate::domain::scene::character::companion::{self, Camera, companion_step};
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
use crate::domain::view::demand;
use crate::domain::view::demand::DemandedBase as _;
use crate::domain::view::frame::coord;
use crate::domain::view::frame::pose;
use crate::domain::view::observation;
use crate::perf::frame::EFFECT_CNT;
use crate::perf::record::{perf_recording, timed};
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
use pxl_spec::space::layer::quadrant::Quadrant;
use xpute_guest::clock::{self, now};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

use crate::spec::phase::Phase;
use crate::sys::sched::param::MAX_STEP_S;

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
/// this frame and not the next.
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
fn demand_tick(_: &mut TickContext) {
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
    match setting::settings().place {
        true => PlaceTile::demand().step(None, &[]),
        false => PlaceTile::demand().idle(),
    }
    EarthTile::demand().step(None, &[]);
    DivisionTile::demand().step(division::line(controller::pivot()), &[]);
    demand::object_step();
}

/// Runs the scene's frame with what control decided. The world gizmo is drawn
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

/// Writes the camera for the last time this frame and composes the view where
/// it now stands. It must run after every writer of the pose (the link's
/// events, the frame's effects, the controllers) and before every phase that
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

/// The tick's steps, phase by phase, in the order they run.
const STEPS: [(Phase, &[Step]); 2] = [
    (
        Phase::Interaction,
        &[
            (StepName::Controller, controller::controller_step),
            (StepName::Input, camera_input_tick),
            (StepName::Camera, camera_tick),
            (StepName::Edit, edit_tick),
            (StepName::Ghost, ghost_tick),
            // The environment runs after the camera, so it sees this tick's
            // camera.
            (StepName::Environment, environment_tick),
            (StepName::Companion, companion_tick),
        ],
    ),
    (
        Phase::Visible,
        &[
            (StepName::Demand, demand_tick),
            (StepName::Scene, scene_tick),
        ],
    ),
];

fn tick(delta: f64) -> TickReport {
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

/// Runs the frame's effects, and returns how long they took and how long each
/// took, indexed by `EffectName`, in milliseconds.
pub(crate) fn effects() -> (f64, [f64; EFFECT_CNT]) {
    let at = now();
    let mut times = perf_recording().then_some([0.0; EFFECT_CNT]);
    // Settings load first, so the frame that reads new settings is also the
    // first to act on them.
    effect!(times, Setting, {
        if setting::settings_load_step() {
            perf_follow(setting::settings().perf);
        }
    });
    effect!(times, Navigation, off_center_step());
    effect!(times, Bootstrap, map_bootstrap_step());
    effect!(times, Follow, follow_mode_step());
    effect!(times, Dst, {
        // The floors the walker found under a position of unknown height
        // are dropped where the ground changed under them.
        support::support_step();
        destination::dst_step();
    });
    effect!(times, Decor, decor_models_step());
    effect!(times, Object, repository::step());
    // The indicator is handed the device's coordinate as reported, None
    // included. It keeps showing its last coordinate while the device briefly
    // reports none.
    effect!(times, Location, location_step(off_center().device_coord));
    effect!(times, Setting, setting::settings_keep_step());
    (now() - at, times.unwrap_or_default())
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
/// the display is measured by (sys/sched/display.rs). That start begins the
/// frame's budget (sys/sched.rs `begin_frame`), and the world's frame runs;
/// once it has run, the entry's tasks get their share (`end_frame`).
#[embassy_executor::task]
async fn frame_task() {
    loop {
        sched::invalidated().await;
        let (display_ms, target_ms) = rate::pace_ms();
        let due_at = rate::due_at(sched::frame_at(), (display_ms, target_ms));
        let frame = vsync::next(clock::mark(due_at)).await.or_bug(Errno::EPROTO);
        display::answered(clock::ms(frame.gap));
        let at = clock::ms(frame.at);
        sched::begin_frame(at, target_ms);
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
            invalidate();
            continue;
        }
        run(at, display_ms, target_ms);
        sched::end_frame();
        clipmap::fill_share();
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
    set_credit_cnt(CREDIT_CNT);
    text_drop_unread();
    let (effects_ms, effects) = effects();
    // A step longer than `MAX_STEP_S` would carry a character through a
    // wall or make a damped camera overshoot.
    let step = delta.min(MAX_STEP_S);
    let report = tick(step);
    let after_tick = now();
    // Requests are pumped once a frame, after every producer has declared its
    // wants: the effects, the tick's tile streams (`demand_tick`), and the
    // assets either of them asked for.
    request::declare(Stream::Asset, loader::load_wants());
    pump();
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
    let frame = stage::stage_render(pace, &write::render_edit());
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
        effects_ms,
        effects,
        stage_ms: end - after_tick,
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
