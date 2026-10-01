// pxl-kernel/world/frame/view.rs

//! The view as drawn: the observer's pose — the eye, and the turn from the
//! world's axes to the camera's — and the matrices made from it, as doubles
//! the renderer, the scheduler's frame, the placement passes and the
//! pointer's ray all read.
//!
//! The world stays in its own axes. Everything is written in the root
//! frame: the world's axes, against the floating origin. A controller makes
//! the pose (camera/orbit.rs `orbit_pose`), and nothing here knows which
//! one, so a consumer reads the eye and the camera's turn and never how a
//! control moved them.
//!
//! Held here the matrices are current the moment a step writes them.

use pxl_spec::space::mat4::{mat4, mat4_frustum_planes, mat4_invert, mat4_mul, mat4_perspective, mat4_transform_direction, mat4_transform_point, mat4_translation, Mat4};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{projection, REACH};
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::controller::{pose, rebase};
use crate::global::Global;
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::fog::{far_clip_at, height_at, near_clip_at};
use crate::world::frame::coord::{local_world_to_canonical, origin_for};
use pxl_spec::space::lod;
use pxl_spec::space::pyramid::TOP_LEVEL;

/// Where the observer is and which way it looks: the eye in the root frame,
/// and the turn from the world's axes to the camera's — right, up and back
/// along the camera's X, Y and Z, rotation alone. Any controller makes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPose {
    pub eye: V3,
    pub rotation: Mat4,
}

pub struct View {
    /// The observer, as the controller last made it.
    pub pose: ObserverPose,
    /// The map origin the root frame is relative to — the floating origin,
    /// as rebased by the translation step.
    pub origin: IVec,

    // ---- the viewport ----
    pub width: f64,
    pub height: f64,
    pub fov: f64,
    pub near: f64,
    pub far: f64,

    // ---- composed ----
    /// Root frame to camera, and back.
    pub view_matrix: Mat4,
    pub inv_view_matrix: Mat4,
    /// Camera to GL clip.
    pub proj: Mat4,
    /// Root frame to GL clip, and back.
    pub local_view_proj: Mat4,
    pub inv_local_view_proj: Mat4,
    /// The frustum's planes in the root frame, normals in.
    pub planes: [f64; 24],
}

impl View {
    fn new() -> View {
        View {
            pose: ObserverPose {
                eye: [0.0, 0.0, 1.0],
                rotation: mat4(),
            },
            origin: 0,
            width: 1.0,
            height: 1.0,
            fov: camera_cfg::projection::FOV,
            near: camera_cfg::projection::NEAR,
            far: render_cfg::FAR_CLIP,
            view_matrix: mat4(),
            inv_view_matrix: mat4(),
            proj: mat4(),
            local_view_proj: mat4(),
            inv_local_view_proj: mat4(),
            planes: [0.0; 24],
        }
    }
}

impl View {
    /// The eye, in units: the root frame's origin and the eye past it.
    pub fn eye_at(&self) -> V3 {
        let o = self.origin;
        let e = self.pose.eye;
        [ivec::to_real(ivec::X(o)) + e[0], ivec::to_real(ivec::Y(o)) + e[1], ivec::to_real(ivec::Z(o)) + e[2]]
    }

    /// The eye's frame to GL clip, and to the camera: the world's axes with
    /// the eye at the origin, where every point the GPU holds is (pxl-render
    /// frame/draw.rs `DrawList::translation`). Nothing of the eye's position
    /// is in either, for a point's to cancel against in floats: the camera is
    /// the pose's turn alone.
    pub fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4();
        mat4_mul(&mut view_proj, &self.proj, &self.pose.rotation);
        (view_proj, self.pose.rotation)
    }

    /// Which way the screen's up points along the ground, as an angle from
    /// east counter-clockwise: the camera's up laid flat.
    pub fn up_heading(&self) -> f64 {
        let r = &self.pose.rotation;
        // The camera's Y in the world's axes is the rotation's second row.
        libm::atan2(r[5], r[1])
    }
}

static VIEW: Global<View> = Global::new();

/// `view`.
pub fn view() -> &'static mut View {
    VIEW.get(View::new)
}

/// Where the flat map is laid back on the planet about, in the root frame:
/// the sea under the eye, so the eye stands where the placement is the
/// identity and the planet's surface is at the sea's height. The sphere is
/// one whichever point it is laid about (pxl-render look/globe.rs), so the
/// anchor is only the frame it is written in, and the eye's is the one where
/// the eye itself needs no placing.
pub fn globe_anchor_local() -> V3 {
    let v = view();
    [v.pose.eye[0], v.pose.eye[1], -ivec::to_real(ivec::Z(v.origin))]
}

/// The globe's anchor in units: what the planet's latitude, its units and
/// the sky over it are taken at. None before the camera stands somewhere.
pub fn globe_anchor() -> Option<Pvf3> {
    crate::control::camera::controller::camera().eye?;
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, &globe_anchor_local(), view().origin);
    Some(out)
}

/// The viewport in CSS pixels; the projection follows it.
pub fn view_resize(width: f64, height: f64) {
    let v = view();
    if width == v.width && height == v.height {
        return;
    }
    v.width = width.max(1.0);
    v.height = height.max(1.0);
    view_compose();
}

/// Composes the pose from the active controller and the matrices from the
/// pose. The floating origin follows the eye, whichever controller put it
/// there: where the eye has left the origin's cell, every controller's
/// positions are rebased onto the eye's, in the same step, so nothing reads
/// a position in one origin and draws it in another.
pub fn view_compose() {
    let v = view();
    v.pose = pose();
    // The origin is a cell of the lattice, which reaches only so high: an eye
    // over it — an orbit drawn far out stands a few times higher — is written
    // against the highest cell it can be, the eye itself being doubles.
    let e = v.eye_at();
    let mut at = Pvf3::default();
    projection::project(&mut at, e[0], e[1], e[2].clamp(REACH.min[2], REACH.max[2] - 1.0));
    let level = lod::level(height_at(v.eye_at())).clamp(0, TOP_LEVEL as i32) as u32;
    let want = origin_for(at.i, level);
    if want != v.origin {
        rebase(crate::world::frame::coord::offset(want, v.origin));
        v.origin = want;
        v.pose = pose();
    }
    let e = v.pose.eye;
    let mut to_eye = mat4();
    mat4_translation(&mut to_eye, -e[0], -e[1], -e[2]);
    mat4_mul(&mut v.view_matrix, &v.pose.rotation, &to_eye);
    let vm = v.view_matrix;
    mat4_invert(&mut v.inv_view_matrix, &vm);
    // The planes follow the reach, which follows the eye's height over the
    // ground under it: no further than the levels the view draws past that
    // ground, nor than the planet's own edge where that is nearer.
    // Read from the view in hand rather than through `eye()`, which would take
    // the view a second time while this holds it.
    let height = height_at(v.eye_at());
    v.near = near_clip_at(height);
    v.far = far_clip_at(height);
    mat4_perspective(&mut v.proj, (v.fov * core::f64::consts::PI) / 180.0, v.width / v.height, v.near, v.far);
    let proj = v.proj;
    mat4_mul(&mut v.local_view_proj, &proj, &vm);
    let lvp = v.local_view_proj;
    mat4_invert(&mut v.inv_local_view_proj, &lvp);
    mat4_frustum_planes(&mut v.planes, &lvp);
}

/// The ray under a screen point, in the root frame: `ndc_x`/`ndc_y` in
/// [−1, 1], x right and y up. `origin` is the eye and `direction` unit.
pub fn view_ray_local(ndc_x: f64, ndc_y: f64, origin: &mut V3, direction: &mut V3) {
    let v = view();
    *origin = v.pose.eye;
    // A point on the far plane, back through the projection and the view.
    let mut far = [0.0; 3];
    mat4_transform_point(&mut far, &v.inv_local_view_proj, ndc_x, ndc_y, 1.0);
    mat4_transform_direction(direction, &mat4(), far[0] - origin[0], far[1] - origin[1], far[2] - origin[2]);
}

/// Whether a sphere in the root frame meets the frustum.
pub fn view_sees_sphere(x: f64, y: f64, z: f64, radius: f64) -> bool {
    let p = &view().planes;
    for k in (0..24).step_by(4) {
        if p[k] * x + p[k + 1] * y + p[k + 2] * z + p[k + 3] < -radius {
            return false;
        }
    }
    true
}
