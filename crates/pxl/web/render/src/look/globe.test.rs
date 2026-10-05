// pxl-render/look/globe.test.rs

use super::*;
use pxl_spec::space::v3::sub3;
use xpute_core::math::rng::{u32_to_unit, SplitMix64};

const SEOUL: f64 = 37.5665 * core::f64::consts::PI / 180.0;

#[test]
fn the_map_center_is_left_where_it_is() {
    let globe = Globe::at(SEOUL);
    assert_eq!(globe.place([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
    // The identity to first order: a unit stays a unit to a millionth.
    for d in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, -1.0, 0.0]] {
        let p = globe.place(d);
        for k in 0..3 {
            assert!((p[k] - d[k]).abs() < 1e-6, "{d:?} -> {p:?}");
        }
    }
}

#[test]
fn a_point_due_north_is_where_its_latitude_is() {
    let globe = Globe::at(SEOUL);
    let north = SEOUL + 2f64.to_radians();
    let dy = MERCATOR_RADIUS_U * (libm::log(libm::tan(core::f64::consts::FRAC_PI_4 + north / 2.0)) - libm::log(libm::tan(core::f64::consts::FRAC_PI_4 + SEOUL / 2.0)));
    let p = globe.place([0.0, dy, 0.0]);
    let r = globe.radius_u();
    let arc = 2f64.to_radians();
    assert!((p[1] - r * libm::sin(arc)).abs() < 1e-3, "{} vs {}", p[1], r * libm::sin(arc));
    assert!((p[2] + r * (1.0 - libm::cos(arc))).abs() < 1e-3, "{} vs {}", p[2], -r * (1.0 - libm::cos(arc)));
    assert!(p[0].abs() < 1e-9);
    // The secant between the two latitudes: about 1.4% at 2°.
    assert!(dy > p[1] * 1.01, "the flat map stretched it: {dy} against {}", p[1]);
}

#[test]
fn every_point_lies_on_the_sphere_at_its_height() {
    let globe = Globe::at(SEOUL);
    let r = globe.radius_u();
    for d in [[1e5, 3e4, 0.0], [-2e5, -1e5, 0.0], [5e4, -3e5, 400.0]] {
        let p = globe.place(d);
        let from_center = (p[0] * p[0] + p[1] * p[1] + (p[2] + r) * (p[2] + r)).sqrt();
        let lat = 2.0 * libm::atan(globe.e0 * libm::exp(d[1] / MERCATOR_RADIUS_U)) - core::f64::consts::FRAC_PI_2;
        let want = r + d[2] * libm::cos(lat) / globe.cos_lat;
        assert!((from_center - want).abs() < 1e-3, "{d:?}: {from_center} vs {want}");
    }
}

#[test]
fn unplace_takes_a_placed_point_back_to_the_map() {
    let globe = Globe::at(SEOUL);
    for d in [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [3e4, -2e4, 700.0], [-4e5, 3e5, 0.0], [2e5, 1e5, 2000.0]] {
        let back = globe.unplace(globe.place(d));
        assert!((back[0] - d[0]).abs() < 1e-2 && (back[1] - d[1]).abs() < 1e-2, "{d:?} came back as {back:?}");
    }
}

/// One sphere whichever point the map is laid about: moving the anchor moves
/// only the frame (same distances, same signed volume).
#[test]
fn the_sphere_is_one_whichever_point_it_is_laid_about() {
    let a = Globe::at(SEOUL);
    let meters_a = a.cos_lat / pxl_spec::space::adapter::epsg3857::u_per_m(0.0);
    let mut rng = SplitMix64::new(0x2545_f491_4f6c_dd1d);
    let mut unit = move || u32_to_unit(rng.next_u32()) - 0.5;
    for _ in 0..200 {
        let off = [unit() * 1.0e5, unit() * 1.0e5];
        let e = a.e0 * libm::exp(off[1] / MERCATOR_RADIUS_U);
        let b = Globe::at(2.0 * libm::atan(e) - core::f64::consts::FRAC_PI_2);
        let meters_b = b.cos_lat / pxl_spec::space::adapter::epsg3857::u_per_m(0.0);
        for _ in 0..8 {
            let span = 3.0e5;
            let pts: [V3; 4] = core::array::from_fn(|_| [unit() * span, unit() * span, (unit() + 0.5) * 2.0e3]);
            let about_b = |v: V3| b.place([v[0] - off[0], v[1] - off[1], v[2]]);
            let (pa, pb): (Vec<V3>, Vec<V3>) = (pts.iter().map(|&p| a.place(p)).collect(), pts.iter().map(|&p| about_b(p)).collect());
            let sub = |x: V3, y: V3| sub3(&x, &y);
            let apart = |x: V3, y: V3| sub(x, y).iter().map(|v| v * v).sum::<f64>().sqrt();
            for (i, j) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
                let (da, db) = (apart(pa[i], pa[j]) * meters_a, apart(pb[i], pb[j]) * meters_b);
                assert!((da - db).abs() <= 1e-9 * da.max(1.0), "{:?} {:?} about {off:?}: {da} m against {db} m", pts[i], pts[j]);
            }
            // Turned, not mirrored.
            let volume = |q: &[V3], m: f64| {
                let (u, v, w) = (sub(q[1], q[0]), sub(q[2], q[0]), sub(q[3], q[0]));
                (u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])) * m * m * m
            };
            let (va, vb) = (volume(&pa, meters_a), volume(&pb, meters_b));
            assert!((va - vb).abs() <= 1e-7 * va.abs().max(1.0), "{pts:?} about {off:?}: a volume of {va} m³ against {vb} m³");
        }
    }
}
