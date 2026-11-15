// pxl-spec/space/coord/adapter/4326.test.rs

use super::*;
use crate::space::coord::geoid::{GEOID_M, GEOID_STEP_DEG};

/// The geoid bake (pxl-tool bin/geoid.rs) writes a row every step from 90°N
/// down to 90°S and a column every step from 0°E east, and the geoid reads it
/// so: each node within the Mercator square's latitudes is its value, and the
/// antimeridian reads alike from either side.
#[test]
fn the_geoid_reads_the_bakes_grid() {
    let step = GEOID_STEP_DEG as usize;
    let (rows, cols) = (180 / step + 1, 360 / step);
    assert_eq!(GEOID_M.len(), rows * cols);
    for r in 0..rows {
        let lat = (90 - (r * step) as i32) as f64;
        if CLAMP_LAT(lat) != lat {
            continue;
        }
        for c in 0..cols {
            let lon = (c * step) as f64;
            assert_eq!(
                geoid_m(lon, lat),
                f64::from(GEOID_M[r * cols + c]),
                "{lat}, {lon}"
            );
        }
    }
    assert_eq!(geoid_m(180.0, 37.5), geoid_m(-180.0, 37.5));
}
