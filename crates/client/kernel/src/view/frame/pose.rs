// pxl-kernel/view/frame/pose.rs

//! The view as drawn: the observer's pose and the matrices made from it,
//! all in the world's doubles (`view/frame/coord.rs`). A controller makes
//! the pose; nothing here knows which.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg4326::{self, G4326};
use pxl_spec::space::coord::mat4::{mat4, mat4_frustum_planes, mat4_invert, mat4_mul, mat4_perspective, mat4_transform_direction, mat4_transform_point, mat4_translation, Mat4};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::nearest;

use crate::control::camera::controller::pose;
use crate::control::navigation::bootstrap::FALLBACK_G4326;
use crate::view::reach::{far_clip_at, near_clip_at};
use pxl_spec::space::coord::globe::Globe;
use xpute_guest::global::Global;

/// `rotation` turns the world's axes to the camera's (right, up, back), rotation alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverPose {
    pub eye: V3,
    pub rotation: Mat4,
}

pub struct View {
    /// The camera as composed this turn; None before it stands anywhere.
    pub pose: Option<ObserverPose>,

    pub width: f64,
    pub height: f64,
    pub fov: f64,
    pub near: f64,
    pub far: f64,

    pub view_matrix: Mat4,
    pub inv_view_matrix: Mat4,
    pub proj: Mat4,
    pub world_view_proj: Mat4,
    pub inv_world_view_proj: Mat4,
    pub frustum: Frustum,
}

impl View {
    fn new() -> View {
        View {
            pose: None,
            width: 1.0,
            height: 1.0,
            fov: crate::param::FOV,
            near: crate::param::NEAR,
            // Set with the camera; reversed depth takes an endless far until then.
            far: f64::INFINITY,
            view_matrix: mat4(),
            inv_view_matrix: mat4(),
            proj: mat4(),
            world_view_proj: mat4(),
            inv_world_view_proj: mat4(),
            frustum: Frustum::default(),
        }
    }
}

impl View {
    /// The pose the matrices are composed at: the camera's, or before it
    /// stands anywhere a rest over the origin, which draws nothing then.
    pub fn at(&self) -> ObserverPose {
        self.pose.unwrap_or(ObserverPose {
            eye: [0.0, 0.0, 1.0],
            rotation: mat4(),
        })
    }

    /// Eye-relative matrices, as the GPU's points are (`DrawList::translation`):
    /// no eye position in either, so nothing large cancels in floats.
    pub fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4();
        let rotation = self.at().rotation;
        mat4_mul(&mut view_proj, &self.proj, &rotation);
        (view_proj, rotation)
    }
}

static VIEW: Global<View> = Global::new();

pub fn view() -> &'static mut View {
    VIEW.get(View::new)
}

/// The sea under the eye: where the globe placement is the identity at the
/// eye. The sphere is the same whichever point it is laid about.
pub fn sea_under_eye() -> V3 {
    let e = view().at().eye;
    [e[0], e[1], 0.0]
}

/// None before the camera stands somewhere. From the doubles: past the north or
/// south edge the lattice would hold the eye at it.
pub fn globe_anchor() -> Option<G4326> {
    let [x, y, _] = view().pose?.eye;
    Some(epsg4326::from_mercator(&[epsg3857::mer_from_u(nearest(x, 0.0)), epsg3857::mer_from_u(y), 0.0]))
}

/// Where the world is laid on the planet: the anchor, or before the camera
/// stands anywhere the place it starts from.
pub fn anchor() -> G4326 {
    globe_anchor().unwrap_or(FALLBACK_G4326)
}

/// The planet the world is laid on, tangent at the anchor.
pub fn globe() -> Globe {
    Globe::at(anchor()[1].to_radians())
}

/// Mercator stretches by the secant of the latitude, heights alike; at the
/// equator it is `u_per_m(0)`.
pub fn km_per_u() -> f64 {
    libm::cos(anchor()[1].to_radians()) / epsg3857::u_per_m(0.0) / 1000.0
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

/// Composes the pose and the matrices.
pub fn view_compose() {
    let v = view();
    v.pose = pose();
    let at = v.at();
    let e = at.eye;
    let mut to_eye = mat4();
    mat4_translation(&mut to_eye, -e[0], -e[1], -e[2]);
    mat4_mul(&mut v.view_matrix, &at.rotation, &to_eye);
    let vm = v.view_matrix;
    mat4_invert(&mut v.inv_view_matrix, &vm);
    let altitude = e[2];
    v.near = near_clip_at(altitude);
    v.far = far_clip_at(altitude);
    mat4_perspective(&mut v.proj, (v.fov * core::f64::consts::PI) / 180.0, v.width / v.height, v.near, v.far);
    let proj = v.proj;
    mat4_mul(&mut v.world_view_proj, &proj, &vm);
    let lvp = v.world_view_proj;
    mat4_invert(&mut v.inv_world_view_proj, &lvp);
    mat4_frustum_planes(&mut v.frustum.planes, &lvp);
}

/// `ndc_x`/`ndc_y` in [−1, 1], x right and y up; `direction` is unit.
pub fn view_ray(ndc_x: f64, ndc_y: f64, origin: &mut V3, direction: &mut V3) {
    let v = view();
    *origin = v.at().eye;
    let mut far = [0.0; 3];
    mat4_transform_point(&mut far, &v.inv_world_view_proj, ndc_x, ndc_y, 1.0);
    mat4_transform_direction(direction, &mat4(), far[0] - origin[0], far[1] - origin[1], far[2] - origin[2]);
}

/// `ndc` in [−1, 1], y up, as logical pixels from the view's top left.
pub fn ndc_to_px([x, y]: [f64; 2]) -> [f64; 2] {
    let v = view();
    [(x * 0.5 + 0.5) * v.width, (-y * 0.5 + 0.5) * v.height]
}
