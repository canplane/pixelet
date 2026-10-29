// pxl-spec/space/coord/adapter/3857.test.rs

use super::*;

/// A 512u square, as a size and nothing more.
const Z15: Zoom = 15;
use crate::space::coord::adapter::epsg4326;
use crate::space::coord::zoom;
use crate::space::coord::{q3, world};

// The round trip goes through the fixed-point lattice; 0.5 is far looser
// than a position's fraction and far tighter than a unit (about 2.389).
const ROUND_TRIP_TOL: f64 = 0.5;

fn half_world() -> f64 {
    merc_from_u(ROUND / 2.0)
}

fn assert_almost(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b}");
}

fn round_trip(x: f64, y: f64, z: f64) -> [f64; 3] {
    let p = project(x, y, z);
    unproject(&p)
}

#[test]
fn round_trip_seoul() {
    // About (127.03, 37.50).
    let x_merc = 14139274.0;
    let y_merc = 4508140.0;
    let [x, y, z] = round_trip(x_merc, y_merc, 50.0);
    assert_almost(x, x_merc, ROUND_TRIP_TOL);
    assert_almost(y, y_merc, ROUND_TRIP_TOL);
    assert_almost(z, 50.0, ROUND_TRIP_TOL);
}

#[test]
fn x_wraps_antimeridian() {
    let p = project(merc_from_u(ROUND) * 1.5 + 1000.0, 0.0, 0.0);
    let out = unproject(&p);
    // 1.5 world widths east wraps to -0.5.
    assert_almost(out[0], -half_world() + 1000.0, ROUND_TRIP_TOL);
}

#[test]
fn north_edge_below_seam() {
    let coord = project(0.0, half_world(), 0.0);

    assert_eq!(coord[1], i32::MAX);

    let out = unproject(&coord);
    assert!(out[1] > 0.0);
    assert_almost(out[1], half_world(), 0.01);
}

#[test]
fn short_way_across_antimeridian() {
    let east = project(half_world() - 1.0, 0.0, 0.0);
    let west = project(-half_world() + 1.0, 0.0, 0.0);
    let delta = world::between(&east, &west);

    assert_almost(merc_from_u(q3::to_real(delta[0])), 2.0, 0.01);
}

#[test]
fn tile_bbox_exclusive_edge() {
    // The last tile before X's positive edge at 2^23 units.
    let last = (1i32 << 23) / zoom::units(Z15) as i32 - 1;
    let tile_idx = [last, last];
    let [_, max] = get_tile_bbox(Z15, tile_idx);

    assert_almost(max[0], half_world(), 1e-8);
    assert_almost(max[1], half_world(), 1e-8);

    let [_, max_geodetic] = epsg4326::get_tile_bbox(Z15, tile_idx);
    assert_almost(max_geodetic[0], 180.0, 1e-10);
}
