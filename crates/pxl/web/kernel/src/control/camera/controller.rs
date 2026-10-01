// pxl-kernel/control/camera/controller.rs

//! The camera, and which controller moves it. The camera is the observer: an
//! eye and a look, (θ, φ) about the world's Z
//! (orbit.rs `look_dir`) — the one state every controller moves, and in the
//! observer's own frame the eye is the origin (world/frame/view.rs). The
//! orbit and the free camera share that geometry and differ in policy: the
//! free camera turns about its eye, first person, and the orbit about a
//! point a radius ahead of it, third person, its `dist`. The map center is
//! that point — the eye first person — derived and never kept (`map_center`);
//! a navigation command puts the camera so the map center is where it says
//! (`put_center`), at once or as every other move comes, as the command
//! says (`Motion`). A controller knows two things only: how its camera's
//! target has a map center, and how its camera is put at one — first
//! person's eye at it, third person's a radius back along the look. A pan (`pan`) walks the map center (layer/terrain/
//! ground_snap.rs `walk`) and puts the camera over where it stands, and a
//! change of person puts the other's camera over the same map center, so
//! neither has a pan or a change of its own. What is drawn comes to the
//! target. A
//! tracker overrides whichever is the base while it holds the pose, which
//! stays as it was under it and is the controller again when the tracker
//! lets go: a tracked turn can roll, and neither base can take a roll as its
//! own.

use pxl_spec::space::mat4::{mat4, mat4_mul, mat4_rotation_z, Mat4};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg::projection::FOV;
use crate::control::camera::config::camera_cfg::view::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR};
use crate::control::camera::free::look_point;
use crate::control::camera::orbit::orbit;
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::global::Global;
use crate::layer::terrain::ground_snap::walk;
use crate::sched::tick::TickContext;
use crate::shell::setting::store::{settings_store, CameraMode};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::local_world_to_canonical;
use crate::world::frame::view::{view, view_compose, ObserverPose};
use xpute_core::math::scalar::wrap;

// ---- the look, in spherical coordinates about the ground's normal ----
//
// Which way a camera looks is a point of the sphere of directions, taken in
// spherical coordinates about the world's Z: θ the angle from straight down
// and φ the heading, from north toward east. θ runs over [0, π] — straight
// down to straight up — and stops there, as the coordinate does; φ is a turn,
// [−π, π), and wraps. Every controller looks by these, and nothing else is a
// way to write a direction.
//
// The coordinates are not one to one with directions, and cannot be: at
// either pole every φ names the one direction, as spherical coordinates do.
// φ is kept there all the same, the heading the look comes back down along
// when it tilts away. Nor is a direction a camera's whole turn: a turn about
// the look itself, a roll, is left, and no rule chooses it smoothly over the
// whole sphere — at the poles the look is the world's up, and nothing is
// across from it. Roll is no control here; the camera's turn is the
// convention `look_rotation` states, under which φ at a pole turns the screen
// about the look rather than the look.

/// A look's coordinates brought into their domain: θ within straight down
/// and straight up, φ wrapped to a turn.
pub fn look_domain(theta: f64, phi: f64) -> (f64, f64) {
    (theta.clamp(0.0, core::f64::consts::PI), wrap(phi, -core::f64::consts::PI, core::f64::consts::PI))
}

/// The unit direction a look at (θ, φ) points along.
pub fn look_dir(theta: f64, phi: f64) -> V3 {
    let (st, ct) = (libm::sin(theta), libm::cos(theta));
    let (sp, cp) = (libm::sin(phi), libm::cos(phi));
    [st * sp, st * cp, -ct]
}

/// The turn from the world's axes to the camera's for a look at (θ, φ), the
/// camera looking along `look_dir`: the world turned by the heading, then
/// about the camera's right by θ — straight down at zero with the heading up
/// the screen, the world's up toward the screen's up between the poles. A
/// rotation at every (θ, φ), the poles included, where it is the heading's
/// turn of the screen alone.
pub fn look_rotation(theta: f64, phi: f64) -> Mat4 {
    let mut heading = mat4();
    mat4_rotation_z(&mut heading, phi);
    let (s, c) = (libm::sin(theta), libm::cos(theta));
    let mut tilt = mat4();
    (tilt[5], tilt[6], tilt[9], tilt[10]) = (c, -s, s, c);
    let mut rotation: Mat4 = mat4();
    mat4_mul(&mut rotation, &tilt, &heading);
    rotation
}

/// How many units a pixel spans at `dist` along the look, on the screen as
/// it is: the view's height there over its height in pixels. A drag that
/// carries the ground this far a pixel keeps what is at that distance under
/// the finger.
pub fn units_per_px_at(dist: f64) -> f64 {
    2.0 * dist * (FOV * core::f64::consts::PI / 360.0).tan() / view().height
}

/// The observer: where the eye is and which way it looks, as drawn. The eye
/// is in the view's root frame, in doubles, and moves with the origin
/// (`rebase`): a canonical point holds a unit's 256th, and the orbit puts
/// the eye back from what it derives of it every frame, so a rounding held
/// in it would walk the map center. None before navigation has put it
/// anywhere.
pub struct Camera {
    pub eye: Option<V3>,
    /// The heading about Z and the angle down from vertical.
    pub yaw: f64,
    pub pitch: f64,
}

static CAMERA: Global<Camera> = Global::new();

pub fn camera() -> &'static mut Camera {
    // Where the controls start, so nothing eases at the first frame.
    CAMERA.get(|| Camera {
        eye: None,
        yaw: DEFAULT_AZIMUTH,
        pitch: DEFAULT_POLAR,
    })
}

/// The eye in the view's root frame.
pub fn eye_local() -> Option<V3> {
    camera().eye
}

/// The eye put at `at`, in the view's root frame.
pub fn set_eye_local(at: &V3) {
    camera().eye = Some(*at);
}

/// How far ahead of the eye, along the look, the active controller turns
/// about: the orbit's distance, and none first person or tracked.
pub fn radius() -> f64 {
    match active() {
        Active::Orbit => orbit().dist,
        Active::Free | Active::Tracked => 0.0,
    }
}

/// The map center in the view's root frame: the camera's center, which is
/// the character's place — `radius` ahead of the drawn eye along its look,
/// the eye itself first person, and the tracked eye while a tracker holds
/// the pose.
pub fn center_local() -> Option<V3> {
    camera().eye?;
    if active() == Active::Tracked {
        return Some(view().pose.eye);
    }
    let (e, c) = (eye_local()?, camera());
    let (d, r) = (look_dir(c.pitch, c.yaw), radius());
    Some(std::array::from_fn(|k| e[k] + d[k] * r))
}

/// The map center: the character's place, and what the world is related to
/// the camera by — derived from the camera, canonical and absolute.
pub fn map_center() -> Option<Pvf3> {
    let at = center_local()?;
    let mut c = Pvf3::default();
    local_world_to_canonical(&mut c, &at, view().origin);
    Some(c)
}

/// How a place said outright is reached: what the command means, never how
/// far it is. A place found elsewhere — a search, the device's fix the
/// recenter names, where the map starts, a change of person — is put at
/// once; a place on the screen or a fix followed is moved to as the hand's
/// moves are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Snap,
    Smooth,
}

/// The camera's target put so the map center is `target`, its look kept, and
/// with `Motion::Snap` the camera there at once. A tracker's pose takes
/// none: its eye is the tracker's, and no base under it is moved in its
/// place.
pub fn put_center(target: Pvf3, motion: Motion) {
    let snap = motion == Motion::Snap;
    match active() {
        Active::Tracked => return,
        Active::Orbit => crate::control::camera::orbit::put(target, snap),
        Active::Free => crate::control::camera::free::put(target, snap),
    }
    view_compose();
    request_world_tick();
}

/// The map center the active controller's target has: where the controls
/// have put it, which the drawn one comes to. None tracked, or before the
/// camera stands anywhere.
fn target_center() -> Option<Pvf3> {
    match active() {
        Active::Orbit => crate::control::camera::orbit::target_center(),
        Active::Free => crate::control::camera::free::target_center(),
        Active::Tracked => None,
    }
}

/// A drag: the map center walked (layer/terrain/ground_snap.rs `walk`) and
/// the camera put over where it stands, whichever controller is active — a
/// controller knows only how a map center is had from its camera and how
/// its camera is put over one. The ground follows the finger at the
/// distance third person looks across, the zoom; first person, which has
/// none, walks as third person does at the distance it starts at
/// (`DEFAULT_DIST`).
pub fn pan(dx: f64, dy: f64) {
    let Some(from) = target_center() else { return };
    let across = match active() {
        Active::Orbit => camera_store().intent.dist,
        Active::Free | Active::Tracked => DEFAULT_DIST,
    };
    let (yaw, upp) = (camera().yaw, units_per_px_at(across));
    let to = walk(&from, dx, dy, upp, yaw);
    let to = match active() {
        Active::Orbit => crate::control::camera::orbit::gate_pan(&to),
        Active::Free | Active::Tracked => to,
    };
    put_center(to, Motion::Smooth);
}

/// Which controller makes the pose now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Active {
    Orbit,
    Free,
    Tracked,
}

/// A tracker holding the pose: the tracker's space placed in the world as
/// it was when the tracker took over, and where the tracker has the camera
/// in it now. Rotations are the camera's axes to the frame's, rotation
/// alone.
struct Tracked {
    world_turn: Mat4,
    world_at: V3,
    turn: Mat4,
    at: V3,
}

struct Controllers {
    base: CameraMode,
    tracked: Option<Tracked>,
    /// How far the view looks, for the pose it was measured at.
    look: Option<(ObserverPose, f64)>,
}

static CONTROLLERS: Global<Controllers> = Global::new();

fn controllers() -> &'static mut Controllers {
    CONTROLLERS.get(|| Controllers {
        base: CameraMode::orbit,
        tracked: None,
        look: None,
    })
}

pub fn active() -> Active {
    let c = controllers();
    match (&c.tracked, c.base) {
        (Some(_), _) => Active::Tracked,
        (None, CameraMode::orbit) => Active::Orbit,
        (None, CameraMode::free) => Active::Free,
    }
}

/// The pose the active controller makes.
pub fn pose() -> ObserverPose {
    let c = controllers();
    match (&c.tracked, c.base) {
        (Some(t), _) => {
            let mut camera_to_world = mat4();
            mul3(&mut camera_to_world, &t.world_turn, &t.turn);
            let moved = apply3(&t.world_turn, t.at);
            ObserverPose {
                eye: std::array::from_fn(|k| t.world_at[k] + moved[k]),
                rotation: transpose3(&camera_to_world),
            }
        }
        (None, _) => {
            let cam = camera();
            ObserverPose {
                eye: eye_local().unwrap_or_default(),
                rotation: look_rotation(cam.pitch, cam.yaw),
            }
        }
    }
}

/// How far the view looks: from the eye to what it looks at along its
/// forward ray — the orbit's radius, and for any other controller what its
/// look falls on (free.rs `look_point`), measured once a pose. What reads it takes the view's reach, whichever controller made it.
pub fn look_distance() -> f64 {
    if active() == Active::Orbit {
        return orbit().dist;
    }
    let now = view().pose;
    let c = controllers();
    if let Some((_, d)) = c.look.filter(|(at, _)| *at == now) {
        return d;
    }
    let r = &now.rotation;
    let forward = [-r[2], -r[6], -r[10]];
    let at = look_point(now.eye, forward, view().far);
    let d = (0..3).map(|k| (at[k] - now.eye[k]).powi(2)).sum::<f64>().sqrt();
    c.look = Some((now, d));
    d
}

/// The camera's eye and a tracker's place moved with the origin, `by` the
/// origin's own move.
pub fn rebase(by: V3) {
    if let Some(e) = &mut camera().eye {
        *e = std::array::from_fn(|k| e[k] - by[k]);
    }
    if let Some(t) = &mut controllers().tracked {
        t.world_at = std::array::from_fn(|k| t.world_at[k] - by[k]);
    }
}

/// The controls ask `partial` of the camera: the intent as far as the active
/// controller takes it — the orbit no further than terrain lets it go
/// (orbit.rs `ask`), the free camera as asked within the controls' ranges.
/// What it took, which a gesture goes on from (gesture.rs).
pub fn set_intent(partial: IntentPatch) -> Intent {
    match active() {
        Active::Orbit => crate::control::camera::orbit::ask(partial),
        Active::Free | Active::Tracked => {
            let c = camera_store();
            c.commit_intent(c.merged(partial));
            c.intent
        }
    }
}

/// North up: the heading the controls ask, back to its default; the tilt
/// is left as the user set it, since "align north" is what both the compass
/// button and the follow mode's end mean.
pub fn reset_azimuth() {
    set_intent(IntentPatch {
        azimuth: Some(DEFAULT_AZIMUTH),
        ..Default::default()
    });
}

/// The distance the controls ask, back to its default.
pub fn reset_dist() {
    set_intent(IntentPatch {
        dist: Some(crate::control::camera::config::camera_cfg::view::DEFAULT_DIST),
        ..Default::default()
    });
}

/// Whether a zoom is in hand — the orbit's alone, as the free camera has
/// none.
pub fn zooming() -> bool {
    active() == Active::Orbit && (crate::control::camera::input::pinching() || crate::control::camera::orbit::zoom_settling())
}

/// A wheel's turn, `delta_y` in CSS pixels: the orbit's zoom. First person
/// has none — the eye stands at its height over the ground, and how high is
/// no control.
pub fn wheel(delta_y: f64) {
    if active() == Active::Orbit {
        crate::control::camera::orbit::wheel(delta_y);
    }
}

/// Two fingers came down `span` pixels apart: where a pinch is measured
/// from.
pub fn pinch_begin(span: f64) {
    crate::control::camera::orbit::pinch_begin(span);
}

/// Two fingers `span` pixels apart: the orbit's zoom, the spread against
/// where the pinch began. First person has none, as the wheel's.
pub fn pinch(span: f64) {
    if active() == Active::Orbit {
        crate::control::camera::orbit::pinch(span);
    }
}

/// The camera brought to `at` to look closer at it: put there, and nearer
/// where it has a nearer to be — the orbit halves its distance.
pub fn focus(at: Pvf3) {
    crate::shell::shell::go_to(at);
    if active() == Active::Orbit {
        crate::control::camera::orbit::zoom_by(0.5);
    }
}

/// The controllers' turn, first in the interaction phase: a change of the
/// settings' camera mode taken: the map center the one had, the other's
/// camera put at it — third person to first, the eye at it; first to
/// third, the eye stepped back by the radius. No earlier
/// height put back and no ground read again, since the map center already
/// stands where the walk left it. The controls are where the camera then
/// is, so nothing eases away.
pub fn controller_step(ctx: &mut TickContext) {
    if switch() {
        ctx.invalidate();
    }
}

/// The camera's turn, after the input's: the active controller's, and only
/// its — which one is decided here and nowhere in them. A tracker's pose is
/// the tracker's and takes no turn.
pub fn camera_step(ctx: &mut TickContext) {
    let moving = match active() {
        Active::Orbit => crate::control::camera::orbit::turn(ctx.delta) == Some(true),
        Active::Free => crate::control::camera::free::turn(ctx.delta),
        Active::Tracked => false,
    };
    if moving {
        ctx.invalidate();
    }
}

/// The change of camera mode the settings ask, taken: whether there was one.
fn switch() -> bool {
    let want = settings_store().camera_mode;
    let c = controllers();
    // A tracker holds the pose, and no base under it is moved in its place
    // (`put_center`): the change is taken once it lets go, at the map center
    // the base has then, so the new camera is never left unplaced.
    if c.base == want || c.tracked.is_some() {
        return false;
    }
    let center = map_center();
    c.base = want;
    let cam = camera();
    if want == CameraMode::orbit {
        camera_store().commit_intent(Intent {
            dist: orbit().dist,
            azimuth: cam.yaw,
            polar: cam.pitch,
        });
    }
    if let Some(center) = center {
        put_center(center, Motion::Snap);
    }
    view_compose();
    true
}

/// A tracked pose: the camera at `at` in the tracker's space, turned by the
/// unit quaternion `q` (x, y, z, w) from the camera's axes to the tracker's.
/// The first of a run places the tracker's space so the camera stays where
/// it is.
pub fn tracked_pose(at: V3, q: [f64; 4]) {
    let turn = quaternion(q);
    let c = controllers();
    if c.tracked.is_none() {
        let now = view().pose;
        // The camera's axes to the world's is the pose's turn reversed; the
        // tracker's space goes in so that where the tracker has the camera
        // now is where the camera is.
        let camera_to_world = transpose3(&now.rotation);
        let mut world_turn = mat4();
        mul3(&mut world_turn, &camera_to_world, &transpose3(&turn));
        let moved = apply3(&world_turn, at);
        c.tracked = Some(Tracked {
            world_turn,
            world_at: std::array::from_fn(|k| now.eye[k] - moved[k]),
            turn,
            at,
        });
    } else if let Some(t) = &mut c.tracked {
        (t.turn, t.at) = (turn, at);
    }
    view_compose();
    request_world_tick();
}

/// The tracker let go: the base controller is the pose again, as it was.
pub fn tracked_end() {
    if controllers().tracked.take().is_some() {
        view_compose();
        request_world_tick();
    }
}

/// The rotation a unit quaternion (x, y, z, w) makes, as the rotation
/// block of a column-major matrix.
fn quaternion([x, y, z, w]: [f64; 4]) -> Mat4 {
    let n = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    let mut m = mat4();
    m[0] = 1.0 - 2.0 * (y * y + z * z);
    m[1] = 2.0 * (x * y + z * w);
    m[2] = 2.0 * (x * z - y * w);
    m[4] = 2.0 * (x * y - z * w);
    m[5] = 1.0 - 2.0 * (x * x + z * z);
    m[6] = 2.0 * (y * z + x * w);
    m[8] = 2.0 * (x * z + y * w);
    m[9] = 2.0 * (y * z - x * w);
    m[10] = 1.0 - 2.0 * (x * x + y * y);
    m
}

/// The rotation blocks of two column-major matrices multiplied, into `out`.
fn mul3(out: &mut Mat4, a: &Mat4, b: &Mat4) {
    let mut m = mat4();
    for c in 0..3 {
        for r in 0..3 {
            m[c * 4 + r] = (0..3).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    *out = m;
}

fn transpose3(m: &Mat4) -> Mat4 {
    let mut t = mat4();
    for c in 0..3 {
        for r in 0..3 {
            t[c * 4 + r] = m[r * 4 + c];
        }
    }
    t
}

fn apply3(m: &Mat4, v: V3) -> V3 {
    std::array::from_fn(|r| (0..3).map(|k| m[k * 4 + r] * v[k]).sum())
}

#[cfg(test)]
#[path = "controller.test.rs"]
mod test;
