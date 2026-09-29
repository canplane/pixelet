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

/// A view as near and as low as the camera comes, turned: written against
/// `origin`, with the map center `center` past it, in units.
fn set(origin: [i64; 3], center: V3) -> &'static View {
    let v = view();
    (v.width, v.height) = (1_000.0, 800.0);
    (v.dist, v.polar, v.azimuth) = (camera_cfg::view::MIN_DIST, camera_cfg::view::MAX_POLAR, 0.7);
    v.origin = ivec::gather(origin[0], origin[1], origin[2]);
    (v.tx, v.ty, v.tz) = (-center[0], -center[1], -center[2]);
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
        let eye_len = v.eye_local.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!(camera[12..15].iter().all(|t| t.abs() <= 1e-9 * eye_len.max(1.0)), "the eye is off the camera: {:?}", &camera[12..15]);
    }
}
