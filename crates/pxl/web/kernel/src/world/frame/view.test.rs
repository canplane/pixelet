// pxl-kernel/world/frame/view.test.rs
//
// The GPU's frame is the eye's: a draw is handed a point less the eye, and
// the matrix takes the eye to the camera. So where a point lands on the
// screen does not depend on where in the world the eye is, nor on which
// origin the view is written against, and a point near the eye is placed as
// finely wherever it is. The GPU is played here as a vertex shader plays
// it: the translation and the matrix narrowed to floats, and multiplied in
// floats.

use pxl_render::frame::draw::DrawList;

use super::*;
use crate::global::serial;
use crate::world::fixture::place;

/// A view as near and as low as the camera comes, turned: written against
/// `origin`, with the map center `center` past it, in units.
fn set(origin: [i64; 3], center: V3) -> &'static View {
    let v = view();
    (v.width, v.height) = (1_000.0, 800.0);
    v.origin = ivec::gather(origin[0], origin[1], origin[2]);
    place(center, camera_cfg::view::MIN_DIST, 1.48, 0.7);
    view_compose();
    v
}

/// The point `d` units from the eye, in units.
fn past_eye(v: &View, d: V3) -> V3 {
    let e = v.eye_at();
    [e[0] + d[0], e[1] + d[1], e[2] + d[2]]
}

/// Where the GPU puts `at`, in normalized device coordinates.
fn on_gpu(v: &View, at: V3) -> [f64; 3] {
    let t = DrawList::translation(at, v.eye_at());
    let m = v.eye_frame().0.map(|x| x as f32);
    let c: [f32; 4] = core::array::from_fn(|r| m[r] * t[0] + m[4 + r] * t[1] + m[8 + r] * t[2] + m[12 + r]);
    [0, 1, 2].map(|k| (c[k] / c[3]) as f64)
}

/// Where `at` is on the screen, in doubles, through the root frame.
fn exact(v: &View, at: V3) -> [f64; 3] {
    let o = v.origin;
    let p = [at[0] - ivec::to_real(ivec::X(o)), at[1] - ivec::to_real(ivec::Y(o)), at[2] - ivec::to_real(ivec::Z(o))];
    let m = v.local_view_proj;
    let c: [f64; 4] = core::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
    [0, 1, 2].map(|k| c[k] / c[3])
}

/// The farthest apart two lists of screen points are, in pixels of `v`.
fn apart(v: &View, a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter().zip(b).map(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1]) * v.height / 2.0).fold(0.0, f64::max)
}

/// Points near the eye, from a tenth of a unit to tens of them.
const NEAR: [V3; 4] = [[0.1, 0.05, -0.02], [0.3, 0.9, -0.1], [1.0, -2.0, -0.5], [40.0, 12.0, -3.0]];

/// The same view moved anywhere in the world puts the same points in the
/// same place.
#[test]
fn where_in_the_world_the_view_is_moves_nothing_on_the_screen() {
    let _g = serial();
    let center = [3_900.5, 777.25, 3.0];
    let here = set([4_096, 8_192, 0], center);
    let a: Vec<_> = NEAR.iter().map(|&d| on_gpu(here, past_eye(here, d))).collect();
    for far in [[1 << 20, -(1 << 21), 0], [-(1 << 22), 1 << 22, 1 << 12]] {
        let there = set([4_096 + far[0], 8_192 + far[1], far[2]], center);
        let b: Vec<_> = NEAR.iter().map(|&d| on_gpu(there, past_eye(there, d))).collect();
        assert!(apart(there, &a, &b) < 1e-3, "{far:?}: moved {} px", apart(there, &a, &b));
    }
}

/// The same view written against another origin puts the same points in
/// the same place: the origin is how the view is written, not where it is.
#[test]
fn the_origin_a_view_is_written_against_moves_nothing_on_the_screen() {
    let _g = serial();
    let (origin, center) = ([4_096, 8_192, 0], [3_900.5, 777.25, 3.0]);
    let here = set(origin, center);
    let points: Vec<_> = NEAR.iter().map(|&d| past_eye(here, d)).collect();
    let a: Vec<_> = points.iter().map(|&p| on_gpu(here, p)).collect();
    for shift in [[4_096, 0, 0], [-8_192, 4_096, 0], [1 << 20, 1 << 20, 0]] {
        let moved = [center[0] - shift[0] as f64, center[1] - shift[1] as f64, center[2] - shift[2] as f64];
        let there = set([origin[0] + shift[0], origin[1] + shift[1], origin[2] + shift[2]], moved);
        let b: Vec<_> = points.iter().map(|&p| on_gpu(there, p)).collect();
        assert!(apart(there, &a, &b) < 1e-3, "{shift:?}: moved {} px", apart(there, &a, &b));
    }
}

/// A point near the eye lands where the doubles put it, to a hundredth of a
/// pixel, with the map center as far from the origin as it gets and the
/// view anywhere in the world; and the eye's frame takes the eye to the
/// camera, with nothing of its position left in the matrix.
#[test]
fn a_point_near_the_eye_is_placed_finely_wherever_the_view_is() {
    let _g = serial();
    for origin in [[0, 0, 0], [1 << 22, -(1 << 22), 1 << 12]] {
        let v = set(origin, [4_095.75, 4_095.5, 3.0]);
        let points: Vec<_> = NEAR.iter().map(|&d| past_eye(v, d)).collect();
        let gpu: Vec<_> = points.iter().map(|&p| on_gpu(v, p)).collect();
        let doubles: Vec<_> = points.iter().map(|&p| exact(v, p)).collect();
        assert!(apart(v, &gpu, &doubles) < 1e-2, "{origin:?}: {} px off", apart(v, &gpu, &doubles));
        let camera = v.eye_frame().1;
        let eye_len = v.pose.eye.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!(camera[12..15].iter().all(|t| t.abs() <= 1e-9 * eye_len.max(1.0)), "the eye is off the camera: {:?}", &camera[12..15]);
    }
}

/// The view as it was composed before the observer owned its pose: the
/// world turned about Z by the heading under a camera fixed south of the
/// pivot, the oracle the pose is held to.
mod before {
    use pxl_spec::space::mat4::{mat4, mat4_frustum_planes, mat4_invert, mat4_look_at, mat4_mul, mat4_perspective, mat4_rotation_z, mat4_transform_point, mat4_translation, Mat4};

    use super::*;
    use crate::layer::environment::fog::{far_clip_at, height_at, near_clip_at};

    pub struct Before {
        pub eye_at: V3,
        pub eye_local: V3,
        pub root: Mat4,
        pub inv_root: Mat4,
        pub local_view_proj: Mat4,
        pub inv_view_proj: Mat4,
        pub planes: [f64; 24],
    }

    pub fn compose(origin: V3, pivot: V3, dist: f64, polar: f64, azimuth: f64, (width, height, fov): (f64, f64, f64)) -> Before {
        let eye = [0.0, -dist * polar.sin(), dist * polar.cos()];
        let mut view_matrix = mat4();
        mat4_look_at(&mut view_matrix, eye, [0.0; 3], [0.0, 0.0, 1.0]);
        let (mut rot, mut trans, mut root, mut inv_root) = (mat4(), mat4(), mat4(), mat4());
        mat4_rotation_z(&mut rot, azimuth);
        mat4_translation(&mut trans, -pivot[0], -pivot[1], -pivot[2]);
        mat4_mul(&mut root, &rot, &trans);
        mat4_invert(&mut inv_root, &root);
        let mut eye_local = [0.0; 3];
        mat4_transform_point(&mut eye_local, &inv_root, eye[0], eye[1], eye[2]);
        let eye_at = [origin[0] + eye_local[0], origin[1] + eye_local[1], origin[2] + eye_local[2]];
        let h = height_at(eye_at);
        let mut proj = mat4();
        mat4_perspective(&mut proj, fov * core::f64::consts::PI / 180.0, width / height, near_clip_at(h), far_clip_at(h));
        let (mut view_proj, mut inv_view_proj, mut local_view_proj) = (mat4(), mat4(), mat4());
        mat4_mul(&mut view_proj, &proj, &view_matrix);
        mat4_invert(&mut inv_view_proj, &view_proj);
        mat4_mul(&mut local_view_proj, &view_proj, &root);
        let mut planes = [0.0; 24];
        mat4_frustum_planes(&mut planes, &view_proj);
        Before {
            eye_at,
            eye_local,
            root,
            inv_root,
            local_view_proj,
            inv_view_proj,
            planes,
        }
    }
}

fn clip(m: &[f64; 16], p: V3) -> [f64; 3] {
    let c: [f64; 4] = core::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
    [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
}

fn inside(planes: &[f64; 24], p: V3, margin: f64) -> Option<bool> {
    let d = planes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|q| (q[0] * p[0] + q[1] * p[1] + q[2] * p[2] + q[3]) / q[0].hypot(q[1]).hypot(q[2]))
        .fold(f64::INFINITY, f64::min);
    (d.abs() > margin).then_some(d > 0.0)
}

/// The observer owning its pose draws what the turned world drew: over
/// pivots about the origin, negative and as far out as Seoul, every
/// distance, tilt and heading, and origins rebased under them, the eye
/// stands where it stood, a point lands on the screen where it landed, a
/// pointer's ray leaves the eye the same way, the frustum holds what it
/// held, and a point laid on the planet about the globe's anchor is seen
/// where it was seen.
#[test]
fn the_observer_s_pose_draws_what_the_turned_world_drew() {
    let _g = serial();
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (width, height) = (1_280.0, 720.0);
    for n in 0..400 {
        let origin = match n % 3 {
            0 => [0, 0, 0],
            1 => [-(1 << 20), 1 << 18, 0],
            _ => [5_911_552, 1_900_544, 0],
        };
        let pivot = [unit() * 800.0 - 400.0, unit() * 800.0 - 400.0, unit() * 60.0 - 10.0];
        let (dist, polar) = (
            camera_cfg::view::MIN_DIST + unit() * 3_000.0,
            // The turned world's look-at stood Z up and came apart straight
            // down and level; the oracle holds only between.
            0.001 + unit() * (1.48 - 0.001),
        );
        let azimuth = unit() * 4.0 * core::f64::consts::PI - 2.0 * core::f64::consts::PI;
        let v = view();
        (v.width, v.height) = (width, height);
        v.origin = ivec::gather(origin[0], origin[1], origin[2]);
        place(pivot, dist, polar, azimuth);
        view_compose();
        let v = view();
        // The view rebases onto the eye's origin as it composes: the same
        // scene, written against where it now is.
        let at = |o: [i64; 3]| o.map(|c| c as f64);
        let moved = [ivec::X(v.origin), ivec::Y(v.origin), ivec::Z(v.origin)];
        let pivot_was = pivot;
        let pivot: V3 = std::array::from_fn(|k| pivot_was[k] + at(origin)[k] - at(moved)[k]);
        let old = before::compose(at(moved), pivot, dist, polar, azimuth, (width, height, v.fov));
        let at = format!("origin {origin:?} pivot {pivot:?} dist {dist} polar {polar} azimuth {azimuth}");

        // The companion's facing, back to the camera: the screen's up laid
        // on the ground, a quarter turn from the heading.
        let turn = (v.up_heading() - core::f64::consts::FRAC_PI_2 + azimuth).rem_euclid(2.0 * core::f64::consts::PI);
        assert!(turn.min(2.0 * core::f64::consts::PI - turn) < 1e-9, "{at}: the screen's up is {} off the heading", turn);

        let eye = v.eye_at();
        assert!((0..3).all(|k| (eye[k] - old.eye_at[k]).abs() < 1e-6), "{at}: the eye {eye:?} against {:?}", old.eye_at);

        for _ in 0..16 {
            // Points about the pivot, out to past the reach.
            let r = dist * (0.2 + unit() * 3.0);
            let p = [pivot[0] + (unit() - 0.5) * r, pivot[1] + (unit() - 0.5) * r, pivot[2] + (unit() - 0.4) * r * 0.3];
            let (a, b) = (clip(&old.local_view_proj, p), clip(&v.local_view_proj, p));
            if a[2].abs() <= 1.0 {
                let px = (a[0] - b[0]).hypot(a[1] - b[1]) * height / 2.0;
                assert!(px < 1e-4, "{at}: {p:?} moved {px} px");
            }
            // The frustum, the old planes in the turned frame.
            let mut q = [0.0; 3];
            pxl_spec::space::mat4::mat4_transform_point(&mut q, &old.root, p[0], p[1], p[2]);
            if let (Some(was), Some(is)) = (inside(&old.planes, q, 1e-6), inside(&v.planes, p, 1e-6)) {
                assert_eq!(was, is, "{at}: {p:?} in the frustum");
            }
            // Laid on the planet about the anchor, as the chunks and the
            // badges measure it: turned back, placed, turned forward before;
            // placed in the world's axes now.
            let globe = pxl_render::look::globe::Globe::at(0.66);
            let (s, c) = (old.root[1], old.root[0]);
            let back = [q[0] * c + q[1] * s, -q[0] * s + q[1] * c, q[2]];
            let laid = globe.place(back);
            let placed_old = [laid[0] * c - laid[1] * s, laid[0] * s + laid[1] * c, laid[2]];
            let d = [p[0] - pivot[0], p[1] - pivot[1], p[2] - pivot[2]];
            let placed = globe.place(d);
            let placed_new = [pivot[0] + placed[0], pivot[1] + placed[1], pivot[2] + placed[2]];
            if let (Some(was), Some(is)) = (inside(&old.planes, placed_old, 1e-6), inside(&v.planes, placed_new, 1e-6)) {
                assert_eq!(was, is, "{at}: {p:?} laid on the planet, in the frustum");
            }
        }

        // The pointer's ray under a few screen points.
        for (x, y) in [(0.0, 0.0), (0.7, -0.4), (-0.9, 0.95)] {
            let (mut from, mut dir) = ([0.0; 3], [0.0; 3]);
            view_ray_local(x, y, &mut from, &mut dir);
            let mut far = [0.0; 3];
            pxl_spec::space::mat4::mat4_transform_point(&mut far, &old.inv_view_proj, x, y, 1.0);
            let f = far;
            pxl_spec::space::mat4::mat4_transform_point(&mut far, &old.inv_root, f[0], f[1], f[2]);
            let want = [far[0] - old.eye_local[0], far[1] - old.eye_local[1], far[2] - old.eye_local[2]];
            let len = want[0].hypot(want[1]).hypot(want[2]);
            let got_len = dir[0].hypot(dir[1]).hypot(dir[2]);
            let cos = (0..3).map(|k| want[k] * dir[k]).sum::<f64>() / (len * got_len);
            assert!(cos > 1.0 - 1e-12, "{at}: the ray at ({x}, {y}) turned by acos {cos}");
            assert!((0..3).all(|k| (from[k] - old.eye_local[k]).abs() < 1e-6), "{at}: the ray starts at {from:?}");
        }
    }
}

/// An orbit drawn far out stands its eye higher than the lattice reaches:
/// the origin stays a cell the lattice has, under the eye, and the eye stands
/// where the orbit puts it, over Seoul and about the origin alike.
#[test]
fn an_eye_over_the_lattice_s_reach_keeps_the_origin_in_it() {
    let _g = serial();
    for at in [[0, 0, 0], [5_911_552, 1_900_544, 0]] {
        let v = view();
        (v.width, v.height) = (1_280.0, 720.0);
        v.origin = ivec::gather(at[0], at[1], at[2]);
        // High enough that the height, wrapped in the lattice's lane, would
        // read as under the ground.
        place([10.0, 20.0, 5.0], 40_000.0, 0.001, 0.3);
        let want = {
            let e = crate::control::camera::orbit::orbit_eye_offset(40_000.0, 0.001, 0.3);
            [at[0] as f64 + 10.0 + e[0], at[1] as f64 + 20.0 + e[1], 5.0 + e[2]]
        };
        view_compose();
        let v = view();
        let z = ivec::to_real(ivec::Z(v.origin));
        assert!((0.0..pxl_spec::space::pvf3::REACH.max[2]).contains(&z), "{at:?}: the origin's height {z}");
        let eye = v.eye_at();
        assert!((0..3).all(|k| (eye[k] - want[k]).abs() < 1e-6), "{at:?}: the eye {eye:?} against {want:?}");
        assert!(want[2] > pxl_spec::space::pvf3::REACH.max[2], "the eye was not over the reach");
    }
}
