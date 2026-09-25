// pxl-kernel/world/frame/view.rs

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
//! (camera/damping.rs, frame/view_frame.rs), and whichever moved
//! composes the matrices again.
//!
//! Held here the matrices are current the moment a step writes them.

use pxl_spec::space::mat4::{
    mat4, mat4_frustum_planes, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_rotation_z, mat4_transform_direction, mat4_transform_point, mat4_translation, Mat4,
};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::orbit::orbit_local_offset;
use crate::global::Global;
use crate::layer::environment::config::render_cfg;
use crate::layer::environment::fog::{far_clip_at, height_at, near_clip_at};
use crate::world::frame::coord::local_world_to_canonical;

const UP: V3 = [0.0, 0.0, 1.0];
const ORIGIN: V3 = [0.0, 0.0, 0.0];

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
    pub eye: V3,
    /// The eye in the root frame, where everything is placed.
    pub eye_local: V3,
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
            dist: camera_cfg::view::DEFAULT_DIST,
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
            eye: [0.0, 0.0, 1.0],
            eye_local: [0.0, 0.0, 1.0],
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

/// The camera centre: the point the orbit looks at, which the world is
/// drawn about and laid on the planet about — the map centre as the view has
/// eased toward it, not the control's target (map_store's map_center), which
/// it trails while it moves. What takes the planet's frame takes it from
/// here — its latitude, its units, the sun and the moon over it — so the
/// sphere's centre and its latitude are one point in the same tick.
pub fn camera_centre() -> Pvf3 {
    let v = view();
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, &[-v.tx, -v.ty, -v.tz], v.origin);
    out
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
    v.eye = [0.0, -o.horiz_dist, o.vert_dist];
    mat4_look_at(&mut v.view_matrix, v.eye, ORIGIN, UP);
    let vm = v.view_matrix;
    mat4_invert(&mut v.inv_view_matrix, &vm);
    let (mut rot, mut trans) = (mat4(), mat4());
    mat4_rotation_z(&mut rot, v.azimuth);
    mat4_translation(&mut trans, v.tx, v.ty, v.tz);
    mat4_mul(&mut v.root, &rot, &trans);
    let root = v.root;
    mat4_invert(&mut v.inv_root, &root);
    let (inv_root, eye) = (v.inv_root, v.eye);
    mat4_transform_point(&mut v.eye_local, &inv_root, eye[0], eye[1], eye[2]);
    // The planes follow the reach, which follows the eye's height over the
    // ground under it: a view laid along the ground reaches no further than
    // the haze, and one from high up no further than the planet's own edge.
    // Read from the view in hand rather than through `eye()`, which would take
    // the view a second time while this holds it.
    let o = v.origin;
    let at = [
        ivec::to_real(ivec::X(o)) + v.eye_local[0],
        ivec::to_real(ivec::Y(o)) + v.eye_local[1],
        ivec::to_real(ivec::Z(o)) + v.eye_local[2],
    ];
    let height = height_at(at);
    v.near = near_clip_at(height);
    v.far = far_clip_at(height);
    mat4_perspective(&mut v.proj, (v.fov * core::f64::consts::PI) / 180.0, v.width / v.height, v.near, v.far);
    let proj = v.proj;
    mat4_mul(&mut v.view_proj, &proj, &vm);
    let vp = v.view_proj;
    mat4_invert(&mut v.inv_view_proj, &vp);
    mat4_mul(&mut v.local_view_proj, &vp, &root);
    mat4_frustum_planes(&mut v.planes, &vp);
}

/// The ray under a screen point, in the root frame: `ndc_x`/`ndc_y` in
/// [−1, 1], x right and y up. `origin` is the eye and `direction` unit.
pub fn view_ray_local(ndc_x: f64, ndc_y: f64, origin: &mut V3, direction: &mut V3) {
    let v = view();
    *origin = v.eye_local;
    // A point on the far plane, back through the projection and the root.
    let mut far = [0.0; 3];
    mat4_transform_point(&mut far, &v.inv_view_proj, ndc_x, ndc_y, 1.0);
    let f = far;
    mat4_transform_point(&mut far, &v.inv_root, f[0], f[1], f[2]);
    mat4_transform_direction(direction, &mat4(), far[0] - origin[0], far[1] - origin[1], far[2] - origin[2]);
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
