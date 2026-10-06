// pxl-kernel/control/camera/controller.rs

//! The camera (an eye and a look) and which controller moves it. The orbit
//! turns about a point `dist` ahead of the eye, the free camera about the eye
//! itself; the map center is that point, derived and never kept. A tracker
//! overrides either while it holds the pose, since a tracked turn can roll.

use pxl_spec::space::coord::mat4::{mat4, mat4_mul, mat4_rotation_quat, mat4_rotation_z, mat4_transform_point, mat4_transpose, Mat4};
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::{add3, sub3, V3};

use crate::control::camera::orbit::orbit;
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::control::navigation::support::{is_unknown, pan_to, unknown_z};
use crate::param::FOV;
use crate::param::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR};
use crate::sched::tick::TickContext;
use crate::shell::setting::store::{settings_store, CameraMode};
use crate::world::change::request_world_tick;
use crate::world::view::frame::coord::{world_to_canonical, world_to_canonical_nearest};
use crate::world::view::frame::pose::{view, view_compose, ObserverPose};
use xpute_core::math::scalar::wrap;
use xpute_guest::global::Global;

// A look in spherical coordinates about the world's Z, θ = polar and
// φ = azimuth: θ from straight down, clamped to [0, π]; φ from north toward
// east, wrapped. At either pole φ still names the direction the look returns
// along. The orbit adds r = dist. There is no roll control.
pub fn look_domain(polar: f64, azimuth: f64) -> (f64, f64) {
    (polar.clamp(0.0, core::f64::consts::PI), wrap(azimuth, -core::f64::consts::PI, core::f64::consts::PI))
}

pub fn look_dir(polar: f64, azimuth: f64) -> V3 {
    let (st, ct) = (libm::sin(polar), libm::cos(polar));
    let (sp, cp) = (libm::sin(azimuth), libm::cos(azimuth));
    [st * sp, st * cp, -ct]
}

/// Straight down at polar 0 with the azimuth up the screen; at the poles the
/// azimuth only turns the screen about the look.
pub fn look_rotation(polar: f64, azimuth: f64) -> Mat4 {
    let mut about_z = mat4();
    mat4_rotation_z(&mut about_z, azimuth);
    let (s, c) = (libm::sin(polar), libm::cos(polar));
    let mut about_x = mat4();
    (about_x[5], about_x[6], about_x[9], about_x[10]) = (c, -s, s, c);
    let mut rotation: Mat4 = mat4();
    mat4_mul(&mut rotation, &about_x, &about_z);
    rotation
}

/// A drag that moves the ground this far a pixel keeps what is at `dist`
/// under the finger.
pub fn units_per_px_at(dist: f64) -> f64 {
    2.0 * dist * libm::tan(FOV * core::f64::consts::PI / 360.0) / view().height
}

/// The eye is kept in the world's doubles: a canonical point's 1/256 rounding
/// held in it would walk the map center as the orbit re-derives the eye each
/// frame.
pub struct Camera {
    pub eye: Option<V3>,
    /// Heading about Z, and the angle down from vertical.
    pub azimuth: f64,
    pub polar: f64,
}

static CAMERA: Global<Camera> = Global::new();

pub fn camera() -> &'static mut Camera {
    // Where the controls start, so nothing eases at the first frame.
    CAMERA.get(|| Camera {
        eye: None,
        azimuth: DEFAULT_AZIMUTH,
        polar: DEFAULT_POLAR,
    })
}

pub fn eye_world() -> Option<V3> {
    camera().eye
}

pub fn set_eye_world(at: &V3) {
    camera().eye = Some(*at);
}

/// Zero first person or tracked.
pub fn dist() -> f64 {
    match active() {
        Active::Orbit => orbit().dist,
        Active::Free | Active::Tracked => 0.0,
    }
}

/// The drawn azimuth, radians from north toward east.
pub fn azimuth() -> f64 {
    camera().azimuth
}

/// Radians from straight down.
pub fn polar() -> f64 {
    camera().polar
}

pub fn center_world() -> Option<V3> {
    camera().eye?;
    if active() == Active::Tracked {
        return Some(view().pose.eye);
    }
    let (e, c) = (eye_world()?, camera());
    let (d, r) = (look_dir(c.polar, c.azimuth), dist());
    Some(std::array::from_fn(|k| e[k] + d[k] * r))
}

pub fn map_center() -> Option<Pvf3> {
    let at = center_world()?;
    let c = world_to_canonical(&at);
    Some(c)
}

/// A place found elsewhere (search, recenter, start, change of person) is
/// snapped to; a place on the screen or a followed fix is eased to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Snap,
    Smooth,
}

/// A tracker's pose ignores it; no base under it moves in its place.
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

fn target_center() -> Option<Pvf3> {
    match active() {
        Active::Orbit => crate::control::camera::orbit::target_center(),
        Active::Free => crate::control::camera::free::target_center(),
        Active::Tracked => None,
    }
}

/// First person pans as third person does at `DEFAULT_DIST`, since it has
/// no zoom distance of its own.
pub fn pan(dx: f64, dy: f64) {
    let Some(from) = target_center() else { return };
    let across = match active() {
        Active::Orbit => camera_store().intent.dist,
        Active::Free | Active::Tracked => DEFAULT_DIST,
    };
    let (azimuth, upp) = (camera().azimuth, units_per_px_at(across));
    let to = pan_to(&from, dx, dy, upp, azimuth);
    put_center(to, Motion::Smooth);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Active {
    Orbit,
    Free,
    Tracked,
}

/// The tracker's space placed in the world as it was at takeover, and the
/// camera in it now.
struct Tracked {
    world_turn: Mat4,
    world_at: V3,
    turn: Mat4,
    at: V3,
}

struct Controllers {
    base: CameraMode,
    tracked: Option<Tracked>,
}

static CONTROLLERS: Global<Controllers> = Global::new();

fn controllers() -> &'static mut Controllers {
    CONTROLLERS.get(|| Controllers {
        base: CameraMode::orbit,
        tracked: None,
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

pub fn pose() -> ObserverPose {
    let c = controllers();
    match (&c.tracked, c.base) {
        (Some(t), _) => {
            let (mut camera_to_world, mut rotation, mut moved) = (mat4(), mat4(), [0.0; 3]);
            mat4_mul(&mut camera_to_world, &t.world_turn, &t.turn);
            mat4_transform_point(&mut moved, &t.world_turn, t.at[0], t.at[1], t.at[2]);
            ObserverPose {
                eye: add3(&t.world_at, &moved),
                rotation: *mat4_transpose(&mut rotation, &camera_to_world),
            }
        }
        (None, _) => {
            let cam = camera();
            ObserverPose {
                eye: eye_world().unwrap_or_default(),
                rotation: look_rotation(cam.polar, cam.azimuth),
            }
        }
    }
}

/// Returns what the controller took, which a gesture continues from.
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

/// Tilt is kept: "align north" is what both the compass and the end of
/// follow mode mean.
pub fn reset_azimuth() {
    set_intent(IntentPatch {
        azimuth: Some(DEFAULT_AZIMUTH),
        ..Default::default()
    });
}

pub fn reset_dist() {
    set_intent(IntentPatch {
        dist: Some(crate::param::DEFAULT_DIST),
        ..Default::default()
    });
}

/// Going to a place: a view far out comes back in; one the user brought
/// nearer stays, as map apps do.
pub fn come_in_to_default() {
    let default = crate::param::DEFAULT_DIST;
    if camera_store().intent.dist > default {
        reset_dist();
    }
}

pub fn zooming() -> bool {
    active() == Active::Orbit && (crate::control::camera::input::pinching() || crate::control::camera::orbit::zoom_settling())
}

/// `delta_y` in CSS pixels. First person has no zoom.
pub fn wheel(delta_y: f64) {
    if active() == Active::Orbit {
        crate::control::camera::orbit::wheel(delta_y);
    }
}

pub fn pinch_begin(span: f64) {
    crate::control::camera::orbit::pinch_begin(span);
}

pub fn pinch(span: f64) {
    if active() == Active::Orbit {
        crate::control::camera::orbit::pinch(span);
    }
}

pub fn controller_step(ctx: &mut TickContext) {
    if switch() {
        ctx.invalidate();
    }
}

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

fn switch() -> bool {
    if !take_mode(settings_store().camera_mode) {
        return false;
    }
    view_compose();
    request_world_tick();
    true
}

/// The other controller's camera is put at the current map center at once,
/// so nothing eases away. While a tracker holds the pose the change waits
/// until it lets go.
fn take_mode(want: CameraMode) -> bool {
    let c = controllers();
    if c.base == want || c.tracked.is_some() {
        return false;
    }
    // Taken back to the nearest canonical point, or a few round trips walk the
    // character away. A height not yet known stays unknown.
    let unknown = target_center().is_some_and(|t| is_unknown(&t));
    let center = center_world().map(|local| {
        let c = world_to_canonical_nearest(&local);
        if unknown {
            unknown_z(&c)
        } else {
            c
        }
    });
    c.base = want;
    let cam = camera();
    if want == CameraMode::orbit {
        camera_store().commit_intent(Intent {
            dist: orbit().dist,
            azimuth: cam.azimuth,
            polar: cam.polar,
        });
    }
    if let Some(center) = center {
        match want {
            CameraMode::orbit => crate::control::camera::orbit::put(center, true),
            CameraMode::free => crate::control::camera::free::put(center, true),
        }
    }
    true
}

/// `q` (x, y, z, w) turns the camera's axes to the tracker's. The first of a
/// run places the tracker's space so the camera stays where it is.
pub fn tracked_pose(at: V3, q: [f64; 4]) {
    let mut turn = mat4();
    mat4_rotation_quat(&mut turn, q);
    let c = controllers();
    if c.tracked.is_none() {
        let now = view().pose;
        let (mut camera_to_world, mut turn_back, mut world_turn, mut moved) = (mat4(), mat4(), mat4(), [0.0; 3]);
        mat4_transpose(&mut camera_to_world, &now.rotation);
        mat4_mul(&mut world_turn, &camera_to_world, mat4_transpose(&mut turn_back, &turn));
        mat4_transform_point(&mut moved, &world_turn, at[0], at[1], at[2]);
        c.tracked = Some(Tracked {
            world_turn,
            world_at: sub3(&now.eye, &moved),
            turn,
            at,
        });
    } else if let Some(t) = &mut c.tracked {
        (t.turn, t.at) = (turn, at);
    }
    view_compose();
    request_world_tick();
}

pub fn tracked_end() {
    if controllers().tracked.take().is_some() {
        view_compose();
        request_world_tick();
    }
}

#[cfg(test)]
#[path = "controller.test.rs"]
mod test;
