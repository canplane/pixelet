// pxl-core/domain/control/camera/orbit.rs

//! The orbit camera. The eye moves on a sphere about the pivot, and the
//! sphere's radius is the zoom. The controls write what the camera is to
//! reach: the intent and the pivot's `to` (state.rs). `step` eases the drawn
//! camera toward them (domain/smooth.rs). Everything that reads the world
//! reads the drawn camera.
//!
//! Every control is written through one function, `ask`. There the terrain
//! answers whether a state puts the eye in the ground (`allowed_view`), and
//! the obstacle setting decides what follows. With `through`, the ground is
//! ignored. With `stop`, a control goes as far as
//! it can. With `climb`, the drawn pitch rises instead (`climb_polar`).
//! Where the way of a control still hits something, a zoom in goes through
//! it (`pull_forward`) and any other control stops (`Step`).
//!
//! How far the orbit gets toward what a control asks before terrain is in
//! the way (`allowed_view`) follows these rules, in the order they are
//! asked:
//!
//! - **Nearest**: within `NEAREST_DIST` of the pivot and above the pivot's
//!   level, the eye goes anywhere, through whatever is there, and the ground
//!   is not asked. The eye's rays do not meet the cell it stands in
//!   (pxl-engine `cast::walk`), so nothing there hides the view. Below the
//!   pivot's level the eye stops as it does anywhere else, and no pitch is
//!   raised. A zoom in tilted below the level can therefore leave the eye in
//!   the ground. From there a move that goes no deeper is let through,
//!   because otherwise no turn, tilt or zoom could ever move the eye again
//!   (`way_hit`).
//! - **Stop**: when the eye's way hits something solid, or would end with
//!   the eye in it, the eye goes as far as it got (`way_hit`). What lies
//!   between the eye and the pivot is not asked.
//! - **Climb**, under `climb`: the way is tested for the eye as it is drawn,
//!   with its pitch raised over the floor under it (`climb_pose`). The eye
//!   rises only where the column over it is open (`rise_clear`). The intent
//!   keeps the pitch that was asked.
//! - **Pull forward**, under `climb`: a zoom in whose climbed way hits something
//!   goes through it to the pivot's side (`pull_forward`). No other control goes
//!   through.
//!
//!   | setting | zoom in | anything else |
//!   | through | as asked | as asked |
//!   | stop | Stop | Stop |
//!   | climb | Climb | Climb |
//!   | climb, its way hit | PullForward | Stop |

use crate::domain::control::camera::state::{
    Intent, IntentPatch, camera_state, camera_state_mut, ease_pivot, resolved_to,
};
use crate::domain::control::camera::{
    camera, ease_facing, eases_toward, facing_dir, facing_domain, set_eye_world,
};
use crate::domain::control::param;
use crate::domain::control::param::{CAM_DAMP, CLIMB_RELAX_DAMP, CLIMB_RISE_DAMP};
use crate::domain::control::param::{CLIMB_PASS_CNT, COLLISION_STEP_SAMPLE_CNT};
use crate::domain::control::param::{EYE_CLEARANCE_U, MIN_DIST, NEAREST_DIST};
use crate::domain::query::change::Memo;
use crate::domain::query::{Hit, Ray, World};
use crate::domain::setting::{OrbitObstacleMode, settings};
use crate::domain::smooth::smooth_vec;
use crate::domain::view::frame::coord::world_to_canonical;
use crate::domain::view::param::NEAR;
use pxl_global::Global;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use xpute_kit::math::scalar::wrap;

pub(crate) struct OrbitState {
    pub dist: f64,
}

static ORBIT: Global<OrbitState> = Global::new();

pub(crate) fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: param::DEFAULT_DIST,
    })
}

pub(crate) fn orbit_eye_off(dist: f64, polar: f64, azimuth: f64) -> V3 {
    facing_dir(polar, azimuth).map(|v| -v * dist)
}

/// The point the orbit turns about, the drawn distance ahead of the eye.
/// None before the camera has an eye.
pub(crate) fn pivot_world() -> Option<V3> {
    let c = camera();
    let e = c.eye?;
    Some(v3::sub(
        &e,
        &orbit_eye_off(orbit().dist, c.polar, c.azimuth),
    ))
}

/// Writes a control into the intent and returns the intent taken. Terrain
/// holds the control back here, where it is written, so a control that
/// turns back moves the camera at once. Under `climb`, the ground is answered
/// by pitching the drawn facing in `step`, because a pitch written into the
/// intent would stay after the ground no longer asks for it.
pub(crate) fn ask(partial: IntentPatch) -> Intent {
    let c = camera_state_mut();
    let asked = c.with_patch(partial);
    c.commit_intent(allowed_view(asked, c.intent));
    c.intent
}

/// A `factor` under one zooms closer.
pub(crate) fn zoom_by(factor: f64) {
    zoom_to(camera_state().intent.dist * factor);
}

/// Returns the distance taken, which terrain may hold short of `dist`.
pub(crate) fn zoom_to(dist: f64) -> f64 {
    ask(IntentPatch {
        dist: Some(dist),
        ..Default::default()
    })
    .dist
}

/// Puts the pivot at its `to` at once, the eye at the drawn distance and
/// facing from it.
pub(crate) fn snap() {
    let at = resolved_to(pivot_world().unwrap_or_default());
    let cam = camera();
    let e = orbit_eye_off(orbit().dist, cam.polar, cam.azimuth);
    set_eye_world(&v3::add(&at, &e));
}

/// Eases the drawn orbit over `delta` seconds and returns whether it is
/// still moving. A camera with no eye has nothing to ease.
pub(crate) fn step(delta: f64) -> bool {
    let Some(drawn_pivot) = pivot_world() else {
        return false;
    };
    let vs = camera_state().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_pivot, o.dist, cam.polar, cam.azimuth);

    let to = resolved_to(drawn_pivot);

    // Under `climb`, the ground pitches the facing toward straight down
    // instead of shortening the distance. The pitch rises fast and relaxes
    // slowly while the ground bounds it.
    let polar = match settings().orbit_obstacle {
        OrbitObstacleMode::Climb => climb_polar(drawn_pivot, o.dist, vs.polar, vs.azimuth),
        _ => vs.polar,
    };
    let rate = match (polar < vs.polar, polar < cam.polar) {
        (true, true) => CLIMB_RISE_DAMP,
        (true, false) => CLIMB_RELAX_DAMP,
        (false, _) => CAM_DAMP,
    };
    let (polar, azimuth) = facing_domain(polar, vs.azimuth);
    let eye_to = v3::add(&to, &orbit_eye_off(vs.dist, polar, azimuth));
    let moving = eases_toward(cam, eye_to, polar, azimuth);
    let at = if moving {
        // The distance eases between two that the intent holds to the
        // camera's range (state.rs `with_patch`), so it stays within it.
        [o.dist] = smooth_vec([o.dist], [vs.dist], CAM_DAMP, delta);
        ease_facing(cam, polar, rate, azimuth, delta);
        ease_pivot(drawn_pivot, to, delta)
    } else {
        (o.dist, cam.polar, cam.azimuth) = (vs.dist, polar, azimuth);
        to
    };

    // The eye is written only when something moved, so a camera at rest is
    // never rewritten.
    if (at, o.dist, cam.polar, cam.azimuth) != was {
        let e = orbit_eye_off(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&v3::add(&at, &e));
    }
    moving
}

/// An orbit pose. It holds the pivot as well as the offset, because the eye
/// is the pivot plus the offset, and a gate that read only the offset could
/// be walked through by panning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewPose {
    pub pivot: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

fn pose(view: Intent) -> Option<ViewPose> {
    Some(ViewPose {
        pivot: pivot_world()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// The floor under the eye of `at` (`World::floor_under`). It is the only
/// floor the gate reads of a pose, and None where nothing is known under the
/// eye.
fn floor_under_eye(at: &ViewPose, world: &World) -> Option<f64> {
    world.floor_under(&eye(at).1).val().flatten()
}

/// The height above the pivot that puts the eye one cell
/// (`EYE_CLEARANCE_U`) over the floor under it, as `floor_under_eye` reads that
/// floor. Ground and placed objects count alike, at any distance. None
/// where nothing is known under the eye.
fn climb_rise(at: &ViewPose, world: &World) -> Option<f64> {
    Some(floor_under_eye(at, world)? + EYE_CLEARANCE_U - at.pivot[2])
}

/// The eye of `at`, in world coordinates and as a canonical point.
fn eye(at: &ViewPose) -> (V3, Q3) {
    let off = orbit_eye_off(at.dist, at.polar, at.azimuth);
    let world: V3 = v3::add(&at.pivot, &off);
    (world, world_to_canonical(&world))
}

/// How far the orbit gets from `from` toward `to`, by the rules at the top
/// of this module.
fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    let mode = settings().orbit_obstacle;
    if mode == OrbitObstacleMode::Through {
        return *to;
    }
    if at_nearest(to) && above_pivot(to) {
        return *to;
    }
    let world = World::now();
    match mode {
        OrbitObstacleMode::Climb => climb_step(to, from, &world),
        _ => match way_hit(from, to, |at| *at, &world) {
            Some(t) => between(from, to, t),
            None => *to,
        },
    }
}

/// The azimuth goes the short way round, as the easing turns it
/// (domain/smooth.rs `smooth_angle`).
fn between(from: &ViewPose, to: &ViewPose, t: f64) -> ViewPose {
    let turn = wrap(
        to.azimuth - from.azimuth,
        -core::f64::consts::PI,
        core::f64::consts::PI,
    );
    ViewPose {
        pivot: std::array::from_fn(|k| from.pivot[k] + (to.pivot[k] - from.pivot[k]) * t),
        dist: from.dist + (to.dist - from.dist) * t,
        polar: from.polar + (to.polar - from.polar) * t,
        azimuth: from.azimuth + turn * t,
    }
}

/// Whether `at` is at the nearest (`NEAREST_DIST`).
fn at_nearest(at: &ViewPose) -> bool {
    at.dist <= NEAREST_DIST
}

/// Whether the eye of `at` is at or above the pivot's level.
fn above_pivot(at: &ViewPose) -> bool {
    at.polar <= core::f64::consts::FRAC_PI_2
}

fn in_solid(at: &ViewPose, world: &World) -> bool {
    floor_under_eye(at, world).is_some_and(|top| top > eye(at).0[2])
}

/// Where a zoom in whose way hit something goes (`Step::PullForward`), at the
/// asked pitch. The eye comes to rest on the pivot's side of the first
/// thing between the pivot and the eye. It rests at the distance the zoom
/// asked, or nearer where that is needed to stay on the pivot's side, but
/// never nearer than `MIN_DIST`. It eases there through the obstacle. None
/// where nothing on the line stands in the way.
fn pull_forward(to: &ViewPose, from: &ViewPose, world: &World) -> Option<ViewPose> {
    let hit = toward_eye(to, from.dist, world)?;
    // The eye stops the near plane's distance short of the obstacle, so the
    // near plane never cuts into it.
    let dist = to.dist.min(hit.t - NEAR).max(MIN_DIST);
    Some(ViewPose { dist, ..*to })
}

/// What a control's step does under `climb`, by the rules at the top of
/// this module.
enum Step {
    /// The step goes as asked, and the drawn pitch rises over what is in the
    /// way (`climb_polar`).
    Climb,
    /// The step stops at this fraction of its way.
    Stop(f64),
    /// The step goes through what is in the way (`pull_forward`).
    PullForward,
}

impl Step {
    fn of(to: &ViewPose, from: &ViewPose, world: &World) -> Step {
        let zoom_in = to.dist < from.dist && to.polar == from.polar && to.azimuth == from.azimuth;
        let way = way_hit(from, to, |at| climb_pose(*at, world), world);
        match way {
            None => Step::Climb,
            Some(_) if zoom_in => Step::PullForward,
            Some(t) => Step::Stop(t),
        }
    }
}

fn climb_step(to: &ViewPose, from: &ViewPose, world: &World) -> ViewPose {
    match Step::of(to, from, world) {
        Step::Climb => *to,
        Step::Stop(t) => between(from, to, t),
        Step::PullForward => pull_forward(to, from, world).unwrap_or(*to),
    }
}

/// How far along the way from `from` to `to` the eye gets before it hits
/// something, as a fraction from 0 to 1: the last sample short of the hit.
/// None where it hits nothing. `draw` turns a pose into the pose as it is
/// drawn. The way is clear when the straight line from the drawn eye to
/// where it ends is clear and the eye ends in nothing solid, both as the
/// observer's eye reads the world. Otherwise the way follows the
/// straight line in control space, as the easing does, and between samples
/// the eye goes up its own column and across (`rise_clear`). An eye that
/// starts in something solid has hit it where it stands. The exception is
/// an eye at the nearest below the pivot's level, which may go where it is
/// no deeper (the rules at the top of this module).
fn way_hit(
    from: &ViewPose,
    to: &ViewPose,
    draw: impl Fn(&ViewPose) -> ViewPose,
    world: &World,
) -> Option<f64> {
    let mut last = draw(from);
    if in_solid(&last, world) {
        let under_nearest = at_nearest(from) && !above_pivot(from);
        let no_deeper = eye(&draw(to)).0[2] >= eye(&last).0[2];
        return (!(under_nearest && no_deeper)).then_some(0.0);
    }
    // Clear where the straight line is, and the eye is in nothing solid
    // where it arrives.
    let arrives = draw(to);
    if line_clear(eye(&last).0, eye(&arrives).0, world) && !in_solid(&arrives, world) {
        return None;
    }
    let (a, b) = (eye(from).0, eye(to).0);
    let moved = libm::sqrt((0..3).map(|k| (b[k] - a[k]) * (b[k] - a[k])).sum());
    let sample_cnt = (libm::ceil(moved / 2.0) as u32).clamp(1, COLLISION_STEP_SAMPLE_CNT);
    for i in 1..=sample_cnt {
        let next = draw(&between(from, to, f64::from(i) / f64::from(sample_cnt)));
        if !rise_clear(&last, &next, world) || in_solid(&next, world) {
            return Some(f64::from(i - 1) / f64::from(sample_cnt));
        }
        last = next;
    }
    None
}

/// Whether the eye gets from `from` to `to` through nothing solid when it
/// moves up its own column, across at the higher of the two heights, and
/// down the column of `to`. The eye can therefore rise only where the
/// column over it is open.
fn rise_clear(from: &ViewPose, to: &ViewPose, world: &World) -> bool {
    let (a, b) = (eye(from).0, eye(to).0);
    let top = a[2].max(b[2]);
    let (over_a, over_b) = ([a[0], a[1], top], [b[0], b[1], top]);
    line_clear(a, over_a, world)
        && line_clear(over_a, over_b, world)
        && line_clear(over_b, b, world)
}

/// Whether the line from `a` to `b` meets nothing a ray cannot see through,
/// read as the eye reads the world (`World::first_hit`). The cell at `a` does not count, and
/// decor never blocks the line.
fn line_clear(a: V3, b: V3, world: &World) -> bool {
    let d = v3::sub(&b, &a);
    let t_max = libm::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if t_max == 0.0 {
        return true;
    }
    let ray = Ray {
        origin: a,
        dir: d.map(|v| v / t_max),
        t_max,
    };
    world.first_hit(&ray).val().flatten().is_none()
}

/// The first thing met on the ray from the pivot of `at` toward its eye,
/// within `t_max`, as `line_clear` reads a line: what a coarser tile stands
/// in for is met too. With the eye straight over the pivot, this is a
/// ceiling. Within the eye's own distance, it is what hides the pivot from
/// the eye.
fn toward_eye(at: &ViewPose, t_max: f64, world: &World) -> Option<Hit> {
    let ray = Ray {
        origin: at.pivot,
        dir: orbit_eye_off(1.0, at.polar, at.azimuth),
        t_max,
    };
    world.first_hit(&ray).val().flatten()
}

/// The intent the orbit can take on its way from `from` toward `to`. Before
/// the camera has a pivot, `to` is taken as asked.
pub(crate) fn allowed_view(to: Intent, from: Intent) -> Intent {
    let (Some(from_pose), Some(to_pose)) = (pose(from), pose(to)) else {
        return to;
    };
    let reached = allowed_step(&to_pose, &from_pose);
    Intent {
        dist: reached.dist,
        polar: reached.polar,
        azimuth: reached.azimuth,
    }
}

/// The shallowest pitch at which the eye clears the terrain, solved as
/// `acos(h / dist)` for a rise `h` over the pivot.
///
/// The answer is kept while the pose and where its pivot is on the world
/// stay the same and the ground it read does not change (`CLIMB_PITCH`).
pub(crate) fn climb_polar(pivot: V3, dist: f64, polar: f64, azimuth: f64) -> f64 {
    let at = ViewPose {
        pivot,
        dist,
        polar,
        azimuth,
    };
    let key = MemoKey {
        at,
        pivot: world_to_canonical(&at.pivot),
    };
    CLIMB_PITCH
        .get(|| Memo::NONE)
        .of(key, || climb_pose(at, &World::now()).polar)
}

/// Drops the kept pitch where a change touched the columns it read. It runs
/// on every frame, so each change is seen exactly once (`change::within`).
pub(crate) fn forget_climb() {
    CLIMB_PITCH.get(|| Memo::NONE).forget_touched();
}

/// What a climb is computed from, compared by value: the pose and where its
/// pivot is on the world.
#[derive(Clone, Copy, PartialEq)]
struct MemoKey {
    at: ViewPose,
    pivot: Q3,
}

/// The pitch of the last climb. It is kept while its `MemoKey` and the eye
/// stay the same and no change touches what it read (`change::Memo`).
static CLIMB_PITCH: Global<Memo<MemoKey, f64>> = Global::new();

/// `at` as the climb draws it. At the nearest (`NEAREST_DIST`), it is drawn
/// as it is. Otherwise its pitch is steepened as the floor under the eye
/// asks (`steepened`), and the steepened pitch is kept even where it did not
/// clear the floor. The asked pitch is kept instead where the eye, at the
/// distance of `at`, would be in something solid even looking straight
/// down, as under the ceiling of a tunnel or a cave.
fn climb_pose(at: ViewPose, world: &World) -> ViewPose {
    if at_nearest(&at) {
        return at;
    }
    let climbed = steepened(at, world);
    match in_solid(&climbed, world) && in_solid(&ViewPose { polar: 0.0, ..at }, world) {
        true => at,
        false => climbed,
    }
}

/// `at` with its pitch steepened until the eye clears the floor under it
/// (`climb_rise`). The pitch only ever steepens. This holds past the
/// horizontal too, where the floor lies below the pivot and the rise is
/// zero or negative.
fn steepened(at: ViewPose, world: &World) -> ViewPose {
    let mut climbed = at;
    for _ in 0..CLIMB_PASS_CNT {
        let Some(rise) = climb_rise(&climbed, world) else {
            break;
        };
        let solved = libm::acos((rise / climbed.dist).clamp(-1.0, 1.0));
        if solved >= climbed.polar {
            break;
        }
        climbed.polar = solved;
    }
    climbed
}

#[cfg(test)]
#[path = "orbit.test.rs"]
mod test;
