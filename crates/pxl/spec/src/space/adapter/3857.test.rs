// pxl-spec/space/adapter/3857.test.rs

use super::*;
use crate::space::adapter::epsg4326;
use crate::space::pvf3::core::FRAC_W;
use crate::space::pvf3::{fvec, ivec, pvf3};

// The round trip goes through the fixed-point lattice; 0.5 is far looser
// than the fraction lane and far tighter than a unit (about 2.389).
const ROUND_TRIP_TOL: f64 = 0.5;

fn half_world() -> f64 {
    to_db(ROUND / 2.0)
}

fn assert_almost(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

fn round_trip(x: f64, y: f64, z: f64) -> [f64; 3] {
    let p = project(x, y, z);
    unproject(p.i, p.f)
}

#[test]
fn epsg3857_project_unproject_round_trips_the_origin() {
    let [x, y, z] = round_trip(0.0, 0.0, 0.0);
    assert_almost(x, 0.0, ROUND_TRIP_TOL);
    assert_almost(y, 0.0, ROUND_TRIP_TOL);
    assert_almost(z, 0.0, ROUND_TRIP_TOL);
}

#[test]
fn epsg3857_project_unproject_round_trips_a_real_world_scale_coordinate_roughly_seoul() {
    // About (127.03, 37.50).
    let x_m = 14139274.0;
    let y_m = 4508140.0;
    let [x, y, _] = round_trip(x_m, y_m, 0.0);
    assert_almost(x, x_m, ROUND_TRIP_TOL);
    assert_almost(y, y_m, ROUND_TRIP_TOL);
}

#[test]
fn epsg3857_project_unproject_round_trips_a_nonzero_altitude_mercator_corrected_and_un_corrected_symmetrically() {
    let [_, _, z] = round_trip(14139274.0, 4508140.0, 50.0);
    assert_almost(z, 50.0, ROUND_TRIP_TOL);
}

#[test]
fn epsg3857_project_wraps_x_across_the_antimeridian_instead_of_growing_unbounded() {
    let p = project(to_db(ROUND) * 1.5 + 1000.0, 0.0, 0.0);
    let out = unproject(p.i, p.f);
    // 1.5 world widths east wraps to -0.5.
    assert_almost(out[0], -half_world() + 1000.0, ROUND_TRIP_TOL);
}

#[test]
fn epsg3857_project_keeps_the_clamped_north_edge_below_the_signed_y_seam() {
    let coord = project(0.0, half_world(), 0.0);

    assert_eq!(ivec::Y(coord.i), (1 << 23) - 1);
    assert_eq!(fvec::Y(coord.f), (1 << FRAC_W) - 1);

    let out = unproject(coord.i, coord.f);
    assert!(out[1] > 0.0);
    assert_almost(out[1], half_world(), 0.01);
}

#[test]
fn epsg3857_project_pvf3_x_subtraction_takes_the_short_antimeridian_path() {
    let east = project(half_world() - 1.0, 0.0, 0.0);
    let west = project(-half_world() + 1.0, 0.0, 0.0);
    let delta = pvf3::sub(&west, &east);

    assert_almost(to_db(pvf3::to_real(pvf3::X(&delta))), 2.0, 0.01);
}

#[test]
fn epsg3857_page_bbox_preserves_the_exclusive_positive_world_boundary() {
    // The last page before X's positive edge at 2^23 units.
    let last = (1i64 << 23) / crate::space::zoom::units(crate::space::pyramid::LEAF_Z) as i64 - 1;
    let page_idx = ivec::gather(last, last, 0);
    let [_, max] = get_page_bbox(page_idx);

    assert_almost(max[0], half_world(), 1e-8);
    assert_almost(max[1], half_world(), 1e-8);

    let [_, max_geodetic] = epsg4326::get_page_bbox(page_idx);
    assert_almost(max_geodetic[0], 180.0, 1e-10);
}

#[test]
fn epsg3857_scale_is_1_at_the_equator_no_mercator_distortion() {
    assert_almost(scale_at(0.0), 1.0, 1e-6);
}

#[test]
fn epsg3857_scale_grows_away_from_the_equator_mercator_s_characteristic_distortion() {
    let at_equator = scale_at(0.0);
    // About 37.5N.
    let at_high_lat = scale_at(from_db(4508140.0));
    assert!(at_high_lat > at_equator);
}
