// pxl-kernel/view/frame/pose.rs

//! The view as drawn: the observer's pose and the matrices made from it,
//! all in the world's doubles (`view/frame/coord.rs`). A controller makes
//! the pose; nothing here knows which.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg4326::{self, G4326};
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::nearest;

use crate::param::FALLBACK_G4326;
use crate::param::FOV;
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
    /// The world surface's drawn pixels a CSS pixel (`resolution::world_ratio`).
    pub ratio: f64,
    /// The quality the eye's rays are cast at (`lens::tau`), of its height
    /// and ratio.
    pub tau: Tau,

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
            // A pixel a CSS pixel until a surface says.
            ratio: 1.0,
            tau: crate::view::lens::tau_of(1.0, 1.0),
            view_matrix: mat4::new(),
            inv_view_matrix: mat4::new(),
            proj: mat4::new(),
            world_view_proj: mat4::new(),
            inv_world_view_proj: mat4::new(),
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
            rotation: mat4::new(),
        })
    }

    /// Eye-relative matrices, as the GPU's points are (`DrawList::translation`):
    /// no eye position in either, so nothing large cancels in floats.
    pub fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4::new();
        let rotation = self.at().rotation;
        mat4::mul(&mut view_proj, &self.proj, &rotation);
        (view_proj, rotation)
    }
}

static VIEW: Global<View> = Global::new();

/// The units a logical pixel spans at `dist` from the eye: what a drag at
/// that distance moves, and what a thing there is worth on the screen.
pub fn units_per_px_at(dist: f64) -> f64 {
    2.0 * dist * libm::tan(FOV * core::f64::consts::PI / 360.0) / view().height
}

pub fn view() -> &'static mut View {
    VIEW.get(View::new)
}

/// The sea under the eye: what is drawn about it in f32 is held to a unit
/// near the eye.
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

/// The eye's longitude and latitude, which the sky is seen from: the
/// anchor, or before the camera stands anywhere the place it starts from.
pub fn anchor() -> G4326 {
    globe_anchor().unwrap_or(FALLBACK_G4326)
}

/// Mercator stretches by the secant of the latitude, heights alike; at the
/// equator it is `u_per_m(0)`.
pub fn km_per_u() -> f64 {
    libm::cos(anchor()[1].to_radians()) / epsg3857::u_per_m(0.0) / 1000.0
}

/// The view `width` × `height` CSS pixels, its world surface drawn at
/// `ratio`: what its matrices and its τ are made of, made again only where
/// one moved.
pub fn view_resize(width: f64, height: f64, ratio: f64) {
    let v = view();
    let (width, height) = (width.max(1.0), height.max(1.0));
    if width == v.width && height == v.height && ratio == v.ratio {
        return;
    }
    (v.width, v.height, v.ratio) = (width, height, ratio);
    v.tau = crate::view::lens::tau_of(height, ratio);
    crate::view::observation::forget();
    compose();
}

/// Takes the pose the controllers hold and composes the matrices from it,
/// where it moved. A turn's observation is the eye's as it stands where the
/// turn composes the view, after the camera's last writer
/// (`observation::observation`), which is what it was where neither the
/// pose nor the ground under the eye moved (`observation::turn`), and is
/// made again where either did.
pub fn view_compose(pose: Option<ObserverPose>) {
    let moved = view().pose != pose;
    crate::view::observation::turn(moved);
    if !moved {
        return;
    }
    view().pose = pose;
    compose();
}

fn compose() {
    let v = view();
    let at = v.at();
    let e = at.eye;
    let mut to_eye = mat4::new();
    mat4::translation(&mut to_eye, -e[0], -e[1], -e[2]);
    mat4::mul(&mut v.view_matrix, &at.rotation, &to_eye);
    let vm = v.view_matrix;
    mat4::invert(&mut v.inv_view_matrix, &vm);
    // Reversed depth in floats with the far plane at infinity: one near
    // plane holds every height's precision.
    mat4::perspective(&mut v.proj, FOV.to_radians(), v.width / v.height, crate::param::NEAR);
    let proj = v.proj;
    mat4::mul(&mut v.world_view_proj, &proj, &vm);
    let lvp = v.world_view_proj;
    mat4::invert(&mut v.inv_world_view_proj, &lvp);
    mat4::frustum_planes(&mut v.frustum.planes, &lvp);
}

/// The ray through `ndc_x`/`ndc_y` in [−1, 1], x right and y up, from
/// `pose`, framed as the view frames its pose (`compose`); `direction` is
/// unit. A pick asks the pose its controller holds, which may have moved
/// since the turn composed the view.
pub fn ray_of(pose: ObserverPose, ndc_x: f64, ndc_y: f64, origin: &mut V3, direction: &mut V3) {
    let v = view();
    let tan = libm::tan(FOV.to_radians() / 2.0);
    // The rotation turns the world's axes to the camera's; a rotation's
    // transpose turns them back.
    let mut back = mat4::new();
    mat4::transpose(&mut back, &pose.rotation);
    *origin = pose.eye;
    mat4::transform_direction(direction, &back, ndc_x * tan * v.width / v.height, ndc_y * tan, -1.0);
}

/// `ndc` in [−1, 1], y up, as logical pixels from the view's top left.
pub fn ndc_to_px([x, y]: [f64; 2]) -> [f64; 2] {
    let v = view();
    [(x * 0.5 + 0.5) * v.width, (-y * 0.5 + 0.5) * v.height]
}

#[cfg(test)]
#[path = "pose.test.rs"]
mod test;
