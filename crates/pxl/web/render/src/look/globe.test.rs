// pxl-render/look/globe.test.rs

use super::*;

const SEOUL: f64 = 37.5665 * core::f64::consts::PI / 180.0;

#[test]
fn the_map_centre_is_left_where_it_is() {
    let globe = Globe::at(SEOUL);
    assert_eq!(globe.place([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
    // A unit out, and a unit up, stay a unit to within a millionth: the
    // placement is the identity to first order.
    for d in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, -1.0, 0.0]] {
        let p = globe.place(d);
        for k in 0..3 {
            assert!((p[k] - d[k]).abs() < 1e-6, "{d:?} -> {p:?}");
        }
    }
}

#[test]
fn a_point_due_north_is_where_its_latitude_is() {
    // A point 2° north of the centre, found by its Mercator ordinate: on the
    // sphere it is 2° of arc away, the arc's chord along north and its
    // sagitta down — where the flat map had it a few percent further off.
    let globe = Globe::at(SEOUL);
    let north = SEOUL + 2f64.to_radians();
    let dy = MERCATOR_RADIUS_U * ((core::f64::consts::FRAC_PI_4 + north / 2.0).tan().ln() - (core::f64::consts::FRAC_PI_4 + SEOUL / 2.0).tan().ln());
    let p = globe.place([0.0, dy, 0.0]);
    let r = globe.radius_u();
    let arc = 2f64.to_radians();
    assert!((p[1] - r * arc.sin()).abs() < 1e-3, "{} vs {}", p[1], r * arc.sin());
    assert!((p[2] + r * (1.0 - arc.cos())).abs() < 1e-3, "{} vs {}", p[2], -r * (1.0 - arc.cos()));
    assert!(p[0].abs() < 1e-9);
    // By the secant between the two latitudes: about 1.4% at 2°.
    assert!(dy > p[1] * 1.01, "the flat map stretched it: {dy} against {}", p[1]);
}

#[test]
fn every_point_lies_on_the_sphere_at_its_height() {
    let globe = Globe::at(SEOUL);
    let r = globe.radius_u();
    for d in [[1e5, 3e4, 0.0], [-2e5, -1e5, 0.0], [5e4, -3e5, 400.0]] {
        let p = globe.place(d);
        let from_centre = (p[0] * p[0] + p[1] * p[1] + (p[2] + r) * (p[2] + r)).sqrt();
        let lat = 2.0 * (globe.e0 * (d[1] / MERCATOR_RADIUS_U).exp()).atan() - core::f64::consts::FRAC_PI_2;
        let want = r + d[2] * lat.cos() / globe.cos_lat;
        assert!((from_centre - want).abs() < 1e-3, "{d:?}: {from_centre} vs {want}");
    }
}
