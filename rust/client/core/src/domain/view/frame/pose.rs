// pxl-core/domain/view/frame/pose.rs

//! The view as drawn: the observer's pose and the matrices made from it,
//! all in the world's doubles (`domain/view/frame/coord.rs`). A camera
//! controller supplies the pose, and nothing here depends on which one.

use pxl_spec::space::camera::eye::Frustum;
use pxl_spec::space::coord::adapter::epsg3857;
use pxl_spec::space::coord::adapter::epsg4326::{self, G4326};
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::q3::{from_real, to_real};
use pxl_spec::space::coord::v3::V3;
use pxl_spec::space::coord::world::nearest;

use crate::domain::view::param;
use crate::domain::view::param::FALLBACK_G4326;
use pxl_global::Global;
use pxl_spec::space::camera::grid::{HALF_TAN, fov};
use pxl_spec::space::coord::adapter::epsg3857::height_u_per_m;

/// `rot` is a pure rotation: it turns the world's axes into the camera's
/// right, up and back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ObserverPose {
    pub eye: V3,
    pub rot: Mat4,
}

pub(crate) struct View {
    /// The pose the view is seen from: the camera's as it was last taken
    /// (`view_compose`), which the camera's own may differ from by less than
    /// can be told apart on the screen. None until the camera stands
    /// somewhere.
    pub pose: Option<ObserverPose>,

    /// The view's size in CSS pixels. Code outside the frame reads it only
    /// through the screen (`domain/view/frame/screen.rs`).
    pub(super) width: f64,
    pub(super) height: f64,

    pub view_mat: Mat4,
    pub inv_view_mat: Mat4,
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
            view_mat: mat4::new(),
            inv_view_mat: mat4::new(),
            proj: mat4::new(),
            world_view_proj: mat4::new(),
            inv_world_view_proj: mat4::new(),
            frustum: Frustum::default(),
        }
    }
}

impl View {
    /// The pose the matrices are composed at. Until the camera stands
    /// somewhere, this is a rest pose one unit over the origin, and nothing
    /// is drawn from it.
    pub(crate) fn at(&self) -> ObserverPose {
        self.pose.unwrap_or(ObserverPose {
            eye: [0.0, 0.0, 1.0],
            rot: mat4::new(),
        })
    }

    /// The view-projection and the rotation relative to the eye, as the GPU's
    /// points are (pxl-render gpu/draw.rs `Position`). Neither holds the
    /// eye's position, so no large values cancel in f32.
    pub(crate) fn eye_frame(&self) -> (Mat4, Mat4) {
        let mut view_proj = mat4::new();
        let rot = self.at().rot;
        mat4::mul(&mut view_proj, &self.proj, &rot);
        (view_proj, rot)
    }
}

static VIEW: Global<View> = Global::new();

pub(crate) fn view() -> &'static View {
    view_mut()
}

pub(crate) fn view_mut() -> &'static mut View {
    VIEW.get(View::new)
}

/// Where the eye stands as the frame composed the view. None until the camera
/// stands somewhere.
pub(crate) fn eye() -> Option<V3> {
    Some(view().pose?.eye)
}

/// The view's heading: radians about Z from north toward east, read off
/// its right, which lies level at any pitch for a camera that does not roll
/// (the orbit and first person; a tracked pose may roll, and has no heading
/// by this). None until the camera stands somewhere.
pub(crate) fn azimuth() -> Option<f64> {
    let rot = view().pose?.rot;
    Some(libm::atan2(-rot[4], rot[0]))
}

/// The point at sea level under the eye, which a frame is drawn about. What
/// is drawn relative to it in f32 stays precise near the eye.
pub(crate) fn sea_under_eye() -> V3 {
    let e = view().at().eye;
    [e[0], e[1], 0.0]
}

/// The eye's longitude and latitude, read from the doubles: the lattice
/// would hold an eye past the north or south edge at that edge. None until
/// the camera stands somewhere.
pub(crate) fn globe_anchor() -> Option<G4326> {
    let [x, y, _] = view().pose?.eye;
    Some(epsg4326::from_mercator(&[
        epsg3857::merc_from_u(nearest(x, 0.0)),
        epsg3857::merc_from_u(y),
        0.0,
    ]))
}

/// The longitude and latitude the sky is seen from: the eye's
/// (`globe_anchor`), or the place the pivot starts from (`FALLBACK_G4326`)
/// until the camera stands somewhere.
pub(crate) fn anchor() -> G4326 {
    globe_anchor().unwrap_or(FALLBACK_G4326)
}

/// How many kilometers one unit of height spans where the eye stands. A
/// unit of height is scaled as the ground is at that latitude (pxl-spec
/// 3857.rs `height_u_per_m`).
pub(crate) fn km_per_u() -> f64 {
    let y_u = epsg3857::u_from_merc(epsg4326::to_mercator(&anchor())[1]);
    1.0 / height_u_per_m(y_u) / 1000.0
}

/// Sets the view to `width` × `height` CSS pixels, the layout box of the
/// world's surface (link/export/surface.rs `resize`), and composes the
/// matrices again when either changed. A pointer's place is read against
/// this size. The pixels the surface is drawn with matter to the drawing
/// alone, since the camera grid is an angle (lens.rs `CELL`).
pub(crate) fn view_resize(width: f64, height: f64) {
    let v = view_mut();
    let (width, height) = (width.max(1.0), height.max(1.0));
    if width == v.width && height == v.height {
        return;
    }
    (v.width, v.height) = (width, height);
    compose();
}

/// Takes the pose the controllers hold where it differs from the view's by
/// what can be told apart, and composes the matrices from it. Returns
/// whether the view moved. The camera moves in reals; the view is what is
/// seen from it, and everything that looks reads the view.
///
/// The eye is snapped to the lattice steps the cast's rays start from
/// (pxl-spec `world::origin`). The world the rays draw and what is
/// rasterized on its faces (the location indicator's zone, the ghosts, the
/// markers' anchors) are then seen from the same eye. The view moves when
/// the snapped eye does, or when the camera has turned from the view's pose
/// by more than `turn_step`; a smaller turn is not taken, so it adds up against
/// the pose the view keeps until it passes.
pub(crate) fn view_compose(pose: Option<ObserverPose>) -> bool {
    let pose = pose.map(|p| ObserverPose {
        eye: p.eye.map(|v| to_real(from_real(v))),
        ..p
    });
    let v = view();
    let moved = match (v.pose, pose) {
        (Some(cur), Some(p)) => cur.eye != p.eye || turn(&cur.rot, &p.rot) > turn_step(v),
        (cur, p) => cur.is_some() != p.is_some(),
    };
    if moved {
        view_mut().pose = pose;
        compose();
    }
    moved
}

/// The angle between two rotations, as 2(1 − cos) of it, which for a small
/// angle is its square: 3 less the trace of one's transpose times the other,
/// over their 3×3 parts.
fn turn(a: &Mat4, b: &Mat4) -> f64 {
    let dot: f64 = [0, 1, 2, 4, 5, 6, 8, 9, 10]
        .iter()
        .map(|&k| a[k] * b[k])
        .sum();
    3.0 - dot
}

/// The turn that moves the screen's corner, where a turn moves a point most,
/// by `TURN_STEP_PX` CSS pixels, in the measure `turn` gives. A point at the
/// tangent `t` from the screen's middle moves 1 + t² times the angle turned.
fn turn_step(v: &View) -> f64 {
    let half_tan = f64::from(HALF_TAN);
    let aspect = v.width / v.height;
    let tan2 = half_tan * half_tan * (1.0 + aspect * aspect);
    let angle = param::TURN_STEP_PX * (2.0 * half_tan / v.height) / (1.0 + tan2);
    2.0 * (1.0 - libm::cos(angle))
}

fn compose() {
    let v = view_mut();
    let at = v.at();
    let e = at.eye;
    let mut to_eye = mat4::new();
    mat4::translation(&mut to_eye, -e[0], -e[1], -e[2]);
    mat4::mul(&mut v.view_mat, &at.rot, &to_eye);
    let vm = v.view_mat;
    mat4::invert(&mut v.inv_view_mat, &vm);
    // The projection uses reversed depth with the far plane at infinity, so
    // one near plane keeps depth precise at every height.
    mat4::perspective(&mut v.proj, fov(), v.width / v.height, param::NEAR);
    let proj = v.proj;
    mat4::mul(&mut v.world_view_proj, &proj, &vm);
    let lvp = v.world_view_proj;
    mat4::invert(&mut v.inv_world_view_proj, &lvp);
    mat4::frustum_planes(&mut v.frustum.planes, &lvp);
}

/// Writes the view's ray through the screen point `ndc_x`, `ndc_y` in
/// [−1, 1], x right and y up, into `origin` and the unit `dir`, so a pick
/// strikes what is shown. The screen's half-angle tangents come from the
/// view's lens as the cast's eye reads them (pxl-spec `Frustum::lens`).
/// Without a screen the ray runs along the view's forward.
pub(crate) fn ray_of(ndc_x: f64, ndc_y: f64, origin: &mut V3, dir: &mut V3) {
    let (pose, tan) = (
        view().at(),
        view().frustum.lens().map_or([0.0; 2], |l| l.tan),
    );
    // The rotation turns the world's axes to the camera's; a rotation's
    // transpose turns them back.
    let mut back = mat4::new();
    mat4::transpose(&mut back, &pose.rot);
    *origin = pose.eye;
    mat4::transform_dir(dir, &back, ndc_x * tan[0], ndc_y * tan[1], -1.0);
}

#[cfg(test)]
#[path = "pose.test.rs"]
mod test;
