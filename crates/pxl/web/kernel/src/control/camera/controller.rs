// pxl-kernel/control/camera/controller.rs

//! The camera (an eye and a look) and which controller moves it. The orbit
//! turns about a point `dist` ahead of the eye, the free camera about the eye
//! itself; the map center is that point, derived and never kept. A tracker
//! overrides either while it holds the pose, since a tracked turn can roll.

use pxl_spec::space::mat4::{mat4, mat4_mul, mat4_rotation_z, Mat4};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg::projection::FOV;
use crate::control::camera::config::camera_cfg::smooth::POS_DAMP;
use crate::control::camera::config::camera_cfg::view::{DEFAULT_AZIMUTH, DEFAULT_POLAR};
use crate::control::camera::free::{hold, look_point};
use crate::control::camera::orbit::orbit;
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::global::Global;
use crate::layer::terrain::ground_snap::ground_or_raw;
use crate::sched::tick::TickContext;
use crate::shell::setting::store::{settings_store, CameraMode};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::{canonical_to_local, local_world_to_canonical};
use crate::world::frame::view::{view, view_compose, ObserverPose};
use xpute_core::math::scalar::wrap;

// A look is (θ, φ) about the world's Z: θ from straight down, clamped to
// [0, π]; φ the heading from north toward east, wrapped. At either pole φ
// still names the heading the look returns along. There is no roll control.
pub fn look_domain(theta: f64, phi: f64) -> (f64, f64) {
    (theta.clamp(0.0, core::f64::consts::PI), wrap(phi, -core::f64::consts::PI, core::f64::consts::PI))
}

pub fn look_dir(theta: f64, phi: f64) -> V3 {
    let (st, ct) = (libm::sin(theta), libm::cos(theta));
    let (sp, cp) = (libm::sin(phi), libm::cos(phi));
    [st * sp, st * cp, -ct]
}

/// Straight down at θ = 0 with the heading up the screen; at the poles φ only
/// turns the screen about the look.
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

/// A drag that moves the ground this far a pixel keeps what is at `dist`
/// under the finger.
pub fn units_per_px_at(dist: f64) -> f64 {
    2.0 * dist * (FOV * core::f64::consts::PI / 360.0).tan() / view().height
}

/// The eye is kept in doubles in the view's root frame and moves with the
/// origin (`rebase`): a canonical point's 1/256 rounding held in it would
/// walk the map center as the orbit re-derives the eye each frame.
pub struct Camera {
    pub eye: Option<V3>,
    /// Heading about Z, and the angle down from vertical.
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

pub fn eye_local() -> Option<V3> {
    camera().eye
}

pub fn set_eye_local(at: &V3) {
    camera().eye = Some(*at);
}

/// Zero first person or tracked.
pub fn radius() -> f64 {
    match active() {
        Active::Orbit => orbit().dist,
        Active::Free | Active::Tracked => 0.0,
    }
}

pub fn center_local() -> Option<V3> {
    camera().eye?;
    if active() == Active::Tracked {
        return Some(view().pose.eye);
    }
    let (e, c) = (eye_local()?, camera());
    let (d, r) = (look_dir(c.pitch, c.yaw), radius());
    Some(std::array::from_fn(|k| e[k] + d[k] * r))
}

pub fn map_center() -> Option<Pvf3> {
    let at = center_local()?;
    let mut c = Pvf3::default();
    local_world_to_canonical(&mut c, &at, view().origin);
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

/// First person walks as third person does at `DEFAULT_DIST`, since it has
/// no zoom distance of its own.
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

/// The orbit's radius, or what the look falls on, measured once per pose.
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

pub fn rebase(by: V3) {
    if let Some(e) = &mut camera().eye {
        *e = std::array::from_fn(|k| e[k] - by[k]);
    }
    if let Some(t) = &mut controllers().tracked {
        t.world_at = std::array::from_fn(|k| t.world_at[k] - by[k]);
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
        dist: Some(crate::control::camera::config::camera_cfg::view::DEFAULT_DIST),
        ..Default::default()
    });
}

/// A view the user brought nearer stays.
pub fn come_in_to_default() {
    let default = crate::control::camera::config::camera_cfg::view::DEFAULT_DIST;
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

pub fn focus(at: Pvf3) {
    crate::shell::shell::go_to(at);
    if active() == Active::Orbit {
        crate::control::camera::orbit::zoom_by(0.5);
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
    if c.base == want {
        return false;
    }
    // Taken back to the nearest canonical point, or a few round trips walk the
    // character away.
    let center = center_local().map(|local| {
        let mut c = Pvf3::default();
        local_world_to_canonical_nearest(&mut c, &local, view().origin);
        c
    });
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
        put_center(if want == CameraMode::orbit { ground_or_raw(&center) } else { center });
    }
    view_compose();
    true
}

/// `q` (x, y, z, w) turns the camera's axes to the tracker's. The first of a
/// run places the tracker's space so the camera stays where it is.
pub fn tracked_pose(at: V3, q: [f64; 4]) {
    let turn = quaternion(q);
    let c = controllers();
    if c.tracked.is_none() {
        let now = view().pose;
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

pub fn tracked_end() {
    if controllers().tracked.take().is_some() {
        view_compose();
        request_world_tick();
    }
}

/// As the rotation block of a column-major matrix.
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
