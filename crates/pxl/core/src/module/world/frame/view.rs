// pxl_core/module/world/frame/view.rs

//! The view as drawn: the camera and the frame the world stands in, as
//! doubles the renderer, the scheduler's frame, the placement passes and the
//! pointer's ray all read.
//!
//! The model is the orbit camera's (camera/orbit.rs): the eye sits at
//! `dist` from the origin, `polar` down from vertical, on the −Y side,
//! looking at the origin with Z up — and never turns. The world turns
//! under it instead: the root frame everything is placed in is the map's
//! azimuth about Z, then a translation that puts the map centre at the
//! origin. Each of those is damped toward its control by its own step
//! (camera/use_camera.rs, frame/use_view_frame.rs), and whichever moved
//! composes the matrices again.
//!
//! Held here the matrices are current the moment a step writes them.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use xpute_core::kit::mat4::{
    mat4, mat4_frustum_planes, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_rotation_z, mat4_transform_direction, mat4_transform_point, mat4_translation, Mat4, Vec3,
};

use crate::module::camera::config::camera_cfg;
use crate::module::camera::orbit::orbit_local_offset;
use crate::module::world::environment::config::render_cfg;
use crate::sys::global::Global;

const UP: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 };
const ORIGIN: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

pub struct View {
    // ---- the controls as drawn, written by their damping steps ----
    /// The eye's distance from the origin and its angle down from vertical,
    /// the climb included.
    pub dist: f64,
    pub polar: f64,
    /// The map's turn about Z.
    pub azimuth: f64,
    /// Where the root frame's origin stands: minus the map centre's offset
    /// from the map origin, in units.
    pub tx: f64,
    pub ty: f64,
    pub tz: f64,
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
    /// The eye, in the view's own frame.
    pub eye: Vec3,
    /// The eye in the root frame, where everything is placed.
    pub eye_local: Vec3,
    /// View-frame to camera, and back: the camera's own matrix.
    pub view_matrix: Mat4,
    pub inv_view_matrix: Mat4,
    /// Camera to GL clip.
    pub proj: Mat4,
    pub view_proj: Mat4,
    pub inv_view_proj: Mat4,
    /// Root frame to view frame, and back.
    pub root: Mat4,
    pub inv_root: Mat4,
    /// Root frame to GL clip.
    pub local_view_proj: Mat4,
    /// The frustum's planes in the view frame, normals in, as the
    /// scheduler's frame takes them.
    pub planes: [f64; 24],
}

impl View {
    fn new() -> View {
        View {
            dist: camera_cfg::view::MAX_DIST,
            polar: 0.0,
            azimuth: 0.0,
            tx: 0.0,
            ty: 0.0,
            tz: 0.0,
            origin: 0,
            width: 1.0,
            height: 1.0,
            fov: camera_cfg::projection::FOV,
            near: camera_cfg::projection::NEAR,
            far: render_cfg::FAR_CLIP,
            eye: Vec3 { x: 0.0, y: 0.0, z: 1.0 },
            eye_local: Vec3 { x: 0.0, y: 0.0, z: 1.0 },
            view_matrix: mat4(),
            inv_view_matrix: mat4(),
            proj: mat4(),
            view_proj: mat4(),
            inv_view_proj: mat4(),
            root: mat4(),
            inv_root: mat4(),
            local_view_proj: mat4(),
            planes: [0.0; 24],
        }
    }
}

static VIEW: Global<View> = Global::new();

/// `view`.
pub fn view() -> &'static mut View {
    VIEW.get(View::new)
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

/// Composes the matrices from the controls as they stand.
pub fn view_compose() {
    let v = view();
    let o = orbit_local_offset(v.dist, v.polar);
    v.eye = Vec3 {
        x: 0.0,
        y: -o.horiz_dist,
        z: o.vert_dist,
    };
    mat4_look_at(&mut v.view_matrix, v.eye, ORIGIN, UP);
    let vm = v.view_matrix;
    mat4_invert(&mut v.inv_view_matrix, &vm);
    mat4_perspective(&mut v.proj, (v.fov * core::f64::consts::PI) / 180.0, v.width / v.height, v.near, v.far);
    let proj = v.proj;
    mat4_mul(&mut v.view_proj, &proj, &vm);
    let vp = v.view_proj;
    mat4_invert(&mut v.inv_view_proj, &vp);
    let (mut rot, mut trans) = (mat4(), mat4());
    mat4_rotation_z(&mut rot, v.azimuth);
    mat4_translation(&mut trans, v.tx, v.ty, v.tz);
    mat4_mul(&mut v.root, &rot, &trans);
    let root = v.root;
    mat4_invert(&mut v.inv_root, &root);
    mat4_mul(&mut v.local_view_proj, &vp, &root);
    mat4_frustum_planes(&mut v.planes, &vp);
    let (inv_root, eye) = (v.inv_root, v.eye);
    mat4_transform_point(&mut v.eye_local, &inv_root, eye.x, eye.y, eye.z);
}

/// The ray under a screen point, in the root frame: `ndc_x`/`ndc_y` in
/// [−1, 1], x right and y up. `origin` is the eye and `direction` unit.
pub fn view_ray_local(ndc_x: f64, ndc_y: f64, origin: &mut Vec3, direction: &mut Vec3) {
    let v = view();
    *origin = v.eye_local;
    // A point on the far plane, back through the projection and the root.
    let mut far = Vec3::default();
    mat4_transform_point(&mut far, &v.inv_view_proj, ndc_x, ndc_y, 1.0);
    let f = far;
    mat4_transform_point(&mut far, &v.inv_root, f.x, f.y, f.z);
    mat4_transform_direction(direction, &mat4(), far.x - origin.x, far.y - origin.y, far.z - origin.z);
}

/// Whether a sphere in the view frame meets the frustum.
pub fn view_sees_sphere(x: f64, y: f64, z: f64, radius: f64) -> bool {
    let p = &view().planes;
    for k in (0..24).step_by(4) {
        if p[k] * x + p[k + 1] * y + p[k + 2] * z + p[k + 3] < -radius {
            return false;
        }
    }
    true
}
