// pxl-kernel/world/frame/view.rs

//! The view as drawn: the observer's pose and the matrices made from it,
//! all in the root frame (the world's axes, against the floating origin). A
//! controller makes the pose; nothing here knows which.

use pxl_spec::space::adapter::epsg3857;
use pxl_spec::space::adapter::epsg4326::{self, G4326};
use pxl_spec::space::mat4::{mat4, mat4_frustum_planes, mat4_invert, mat4_mul, mat4_perspective, mat4_transform_direction, mat4_transform_point, mat4_translation, Mat4};
use pxl_spec::space::observation::Frustum;
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::nearest;
use pxl_spec::space::pvf3::{projection, ROUND};
use pxl_spec::space::v3::{add3, V3};
use xpute_core::math::scalar::round;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::controller::{pose, rebase};
use crate::layer::environment::fog::{far_clip_at, height_at, near_clip_at};
use crate::world::frame::coord::origin_for;
use pxl_spec::space::lod::Tau;
use xpute_runtime::global::Global;

/// `rotation` turns the world's axes to the camera's (right, up, back), rotation alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPose {
    pub eye: V3,
    pub rotation: Mat4,
}

pub struct View {
    pub pose: ObserverPose,
    /// The floating origin, stored as PVF3 names it.
    pub origin: IVec,
    /// Rounds the origin has gone: the view's frame runs on across the antimeridian.
    lap: i64,

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
    pub frustum: Frustum,
}

impl View {
    fn new() -> View {
        View {
            pose: ObserverPose {
                eye: [0.0, 0.0, 1.0],
                rotation: mat4(),
            },
            origin: 0,
            lap: 0,
            width: 1.0,
            height: 1.0,
            fov: camera_cfg::projection::FOV,
            near: camera_cfg::projection::NEAR,
            // Set with the camera; reversed depth takes an endless far until then.
            far: f64::INFINITY,
            view_matrix: mat4(),
            inv_view_matrix: mat4(),
            proj: mat4(),
            local_view_proj: mat4(),
            inv_local_view_proj: mat4(),
            frustum: Frustum::default(),
        }
    }
}

impl View {
    /// The origin in the view's frame: X counted on round the world.
    pub fn chart(&self) -> V3 {
        let [x, y, z] = ivec::to_v3(self.origin);
        [x + self.lap as f64 * ROUND, y, z]
    }

    pub fn eye_at(&self) -> V3 {
        add3(&self.chart(), &self.pose.eye)
    }

    /// The zoom the origin's cell is sized for (`origin_for`).
    pub fn anchor_z(&self) -> u32 {
        Tau::FINEST.z_at(height_at(self.eye_at()))
    }

    /// Eye-relative matrices, as the GPU's points are (`DrawList::translation`):
    /// no eye position in either, so nothing large cancels in floats.
    pub fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4();
        mat4_mul(&mut view_proj, &self.proj, &self.pose.rotation);
        (view_proj, self.pose.rotation)
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

/// None before the camera stands somewhere. From the doubles: past the north or
/// south edge the lattice would hold the eye at it.
pub fn globe_anchor() -> Option<G4326> {
    crate::control::camera::controller::camera().eye?;
    let [x, y, _] = add3(&view().chart(), &globe_anchor_local());
    Some(epsg4326::from_mercator(&[epsg3857::to_db(nearest(x, 0.0)), epsg3857::to_db(y), 0.0]))
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
    // Past the lattice's edges, against the edge cell.
    let e = v.eye_at();
    let at = projection::project(e[0], e[1], e[2]);
    let want = origin_for(at.i, v.anchor_z());
    if want != v.origin {
        let by = crate::world::frame::coord::offset(want, v.origin);
        let x = v.chart()[0] + by[0];
        v.lap = round((x - ivec::to_real(ivec::X(want))) / ROUND) as i64;
        rebase(by);
        v.origin = want;
        v.pose = pose();
    }
    let e = v.pose.eye;
    let mut to_eye = mat4();
    mat4_translation(&mut to_eye, -e[0], -e[1], -e[2]);
    mat4_mul(&mut v.view_matrix, &v.pose.rotation, &to_eye);
    let vm = v.view_matrix;
    mat4_invert(&mut v.inv_view_matrix, &vm);
    // Read the eye from the view in hand: `eye()` would borrow the view again.
    let altitude = v.eye_at()[2];
    v.near = near_clip_at(altitude);
    v.far = far_clip_at(altitude);
    mat4_perspective(&mut v.proj, (v.fov * core::f64::consts::PI) / 180.0, v.width / v.height, v.near, v.far);
    let proj = v.proj;
    mat4_mul(&mut v.local_view_proj, &proj, &vm);
    let lvp = v.local_view_proj;
    mat4_invert(&mut v.inv_local_view_proj, &lvp);
    mat4_frustum_planes(&mut v.frustum.planes, &lvp);
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
