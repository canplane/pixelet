// pxl-shared/spec/space/adapter/3857.test.rs

use super::*;
use crate::spec::space::adapter::epsg4326;
use crate::spec::space::pvf3::{fvec, ivec, pvf3};

// project()/unproject() round-trip through a fixed-point PVF3 lattice (see
// pvf3/projection.rs), so exact equality isn't expected — only that the
// error stays within one lattice cell's worth of meters. 1u ≈ 2.389m (see
// M_PER_U in 3857.rs) and the fractional lane resolves well below that, so a
// tolerance far looser than a full unit (0.5m) confirms the round-trip isn't
// silently degenerating, without being so tight it starts asserting on the
// lattice's own intentional quantization.
const ROUND_TRIP_TOL_M: f64 = 0.5;
const WORLD_WIDTH_M: f64 = 40075016.68557849;
const HALF_WORLD_M: f64 = WORLD_WIDTH_M / 2.0;

fn assert_almost(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

fn round_trip(x: f64, y: f64, z: f64) -> [f64; 3] {
    let mut p = PVF3::default();
    project(&mut p, x, y, z);
    let mut out: G3857 = [0.0; 3];
    unproject(&mut out, p.i, p.f);
    out
}

#[test]
fn epsg3857_project_unproject_round_trips_the_origin() {
    let [x, y, z] = round_trip(0.0, 0.0, 0.0);
    assert_almost(x, 0.0, ROUND_TRIP_TOL_M);
    assert_almost(y, 0.0, ROUND_TRIP_TOL_M);
    assert_almost(z, 0.0, ROUND_TRIP_TOL_M);
}

#[test]
fn epsg3857_project_unproject_round_trips_a_real_world_scale_coordinate_roughly_seoul() {
    // EPSG:3857 meters for lon/lat ~ (127.03, 37.50).
    let x_m = 14139274.0;
    let y_m = 4508140.0;
    let [x, y, _] = round_trip(x_m, y_m, 0.0);
    assert_almost(x, x_m, ROUND_TRIP_TOL_M);
    assert_almost(y, y_m, ROUND_TRIP_TOL_M);
}

#[test]
fn epsg3857_project_unproject_round_trips_a_nonzero_altitude_mercator_corrected_and_un_corrected_symmetrically() {
    let [_, _, z] = round_trip(14139274.0, 4508140.0, 50.0);
    assert_almost(z, 50.0, ROUND_TRIP_TOL_M);
}

#[test]
fn epsg3857_project_wraps_x_across_the_antimeridian_instead_of_growing_unbounded() {
    let mut p = PVF3::default();
    project(&mut p, WORLD_WIDTH_M * 1.5, 0.0, 0.0);
    let mut out: G3857 = [0.0; 3];
    unproject(&mut out, p.i, p.f);
    // 1.5 world-widths east wraps back to -0.5 world-widths (i.e. the
    // antimeridian's other side), not 1.5 world-widths.
    assert_almost(out[0], -HALF_WORLD_M, ROUND_TRIP_TOL_M);
}

#[test]
fn epsg3857_scale_maps_one_complete_world_onto_the_full_24_bit_x_lane() {
    assert_almost(M_PER_U, WORLD_WIDTH_M / (1u64 << 24) as f64, 1e-12);
    assert_almost(U_PER_M, (1u64 << 24) as f64 / WORLD_WIDTH_M, 1e-12);
}

#[test]
fn epsg3857_project_keeps_the_clamped_north_edge_below_the_signed_y_seam() {
    let mut coord = PVF3::default();
    project(&mut coord, 0.0, HALF_WORLD_M, 0.0);

    assert_eq!(ivec::Y(coord.i), (1 << 23) - 1);
    assert_eq!(fvec::Y(coord.f), (1 << FRAC_W) - 1);

    let mut out: G3857 = [0.0; 3];
    unproject(&mut out, coord.i, coord.f);
    assert!(out[1] > 0.0);
    assert_almost(out[1], HALF_WORLD_M, 0.01);
}

#[test]
fn epsg3857_project_pvf3_x_subtraction_takes_the_short_antimeridian_path() {
    let (mut east, mut west, mut delta) = (PVF3::default(), PVF3::default(), PVF3::default());

    project(&mut east, HALF_WORLD_M - 1.0, 0.0, 0.0);
    project(&mut west, -HALF_WORLD_M + 1.0, 0.0, 0.0);
    pvf3::sub(&mut delta, &west, &east);

    assert_almost(pvf3::to_real(pvf3::X(&delta)) * M_PER_U, 2.0, 0.01);
}

#[test]
fn epsg3857_page_bbox_preserves_the_exclusive_positive_world_boundary() {
    let page_idx = ivec::gather(8191, 8191, 0);
    let [_, max] = get_page_bbox(page_idx);

    assert_almost(max[0], HALF_WORLD_M, 1e-8);
    assert_almost(max[1], HALF_WORLD_M, 1e-8);

    let [_, max_geodetic] = epsg4326::get_page_bbox(page_idx);
    assert_almost(max_geodetic[0], 180.0, 1e-10);
}

#[test]
fn epsg3857_z_scale_is_1_at_the_equator_no_mercator_distortion() {
    assert_almost(Z_SCALE(0.0), 1.0, 1e-6);
}

#[test]
fn epsg3857_z_scale_grows_away_from_the_equator_mercator_s_characteristic_distortion() {
    let at_equator = Z_SCALE(0.0);
    // ~37.5N in EPSG:3857 meters.
    let at_high_lat = Z_SCALE(4508140.0);
    assert!(at_high_lat > at_equator);
}

#[test]
fn epsg3857_z_scale_constant_within_one_l3_page_the_documented_coarse_scale_contract_not_a_bug() {
    // Z_SCALE deliberately snaps to the *page base* before evaluating scale
    // (see its doc comment in 3857.rs) so terrain/object/baking never drift
    // from each other by mixing slightly different per-point scales — two
    // points a few meters apart, comfortably inside the same page, must
    // return bit-identical scale, not just approximately equal.
    let base = 4508140.0;
    assert_eq!(Z_SCALE(base), Z_SCALE(base + 3.0));
}
