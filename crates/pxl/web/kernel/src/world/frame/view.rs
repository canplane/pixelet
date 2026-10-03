// pxl-kernel/world/frame/view.rs

//! The view as drawn: the observer's pose and the matrices made from it,
//! all in the root frame (the world's axes, against the floating origin). A
//! controller makes the pose; nothing here knows which.

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

/// `rotation` turns the world's axes to the camera's (right, up, back), rotation alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPose {
    pub eye: V3,
    pub rotation: Mat4,
}

pub struct View {
    pub pose: ObserverPose,
    pub origin: IVec,

    pub width: f64,
    pub height: f64,
    pub fov: f64,
    pub near: f64,
    pub far: f64,

    pub view_matrix: Mat4,
    pub inv_view_matrix: Mat4,
    pub proj: Mat4,
    pub local_view_proj: Mat4,
    pub inv_local_view_proj: Mat4,
    /// Normals in.
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
    pub fn eye_at(&self) -> V3 {
        let o = self.origin;
        let e = self.pose.eye;
        [ivec::to_real(ivec::X(o)) + e[0], ivec::to_real(ivec::Y(o)) + e[1], ivec::to_real(ivec::Z(o)) + e[2]]
    }

    /// Eye-relative matrices, as the GPU's points are (`DrawList::translation`):
    /// no eye position in either, so nothing large cancels in floats.
    pub fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4();
        mat4_mul(&mut view_proj, &self.proj, &self.pose.rotation);
        (view_proj, self.pose.rotation)
    }

    /// The camera's up laid flat on the ground, as an angle counter-clockwise from east.
    pub fn up_heading(&self) -> f64 {
        let r = &self.pose.rotation;
        // The camera's Y in the world's axes is the rotation's second row.
        libm::atan2(r[5], r[1])
    }
}

static VIEW: Global<View> = Global::new();

pub fn view() -> &'static mut View {
    VIEW.get(View::new)
}

/// The sea under the eye: where the globe placement is the identity at the
/// eye. The sphere is the same whichever point it is laid about.
pub fn globe_anchor_local() -> V3 {
    let v = view();
    [v.pose.eye[0], v.pose.eye[1], -ivec::to_real(ivec::Z(v.origin))]
}

/// None before the camera stands somewhere.
pub fn globe_anchor() -> Option<Pvf3> {
    crate::control::camera::controller::camera().eye?;
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, &globe_anchor_local(), view().origin);
    Some(out)
}

pub fn view_resize(width: f64, height: f64) {
    let v = view();
    if width == v.width && height == v.height {
        return;
    }
    v.width = width.max(1.0);
    v.height = height.max(1.0);
    view_compose();
}

/// Composes the pose and the matrices. When the eye leaves the origin's
/// cell, every controller is rebased in the same step, so nothing reads a
/// position in one origin and draws it in another.
pub fn view_compose() {
    let v = view();
    v.pose = pose();
    // The lattice only reaches so high: an eye above it is written against the
    // highest cell, the eye itself being doubles.
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
    // Read the height from the view in hand: `eye()` would borrow the view again.
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

/// `ndc_x`/`ndc_y` in [−1, 1], x right and y up; `direction` is unit.
pub fn view_ray_local(ndc_x: f64, ndc_y: f64, origin: &mut V3, direction: &mut V3) {
    let v = view();
    *origin = v.pose.eye;
    let mut far = [0.0; 3];
    mat4_transform_point(&mut far, &v.inv_local_view_proj, ndc_x, ndc_y, 1.0);
    mat4_transform_direction(direction, &mat4(), far[0] - origin[0], far[1] - origin[1], far[2] - origin[2]);
}

/// `ndc` in [−1, 1], y up, as logical pixels from the view's top left.
pub fn ndc_to_px([x, y]: [f64; 2]) -> [f64; 2] {
    let v = view();
    [(x * 0.5 + 0.5) * v.width, (-y * 0.5 + 0.5) * v.height]
}

pub fn view_sees_sphere(x: f64, y: f64, z: f64, radius: f64) -> bool {
    let p = &view().planes;
    for k in (0..24).step_by(4) {
        if p[k] * x + p[k + 1] * y + p[k + 2] * z + p[k + 3] < -radius {
            return false;
        }
    }
    true
}
