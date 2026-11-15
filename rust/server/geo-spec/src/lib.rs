// pxl-geo-spec/lib.rs

//! The db-geo table names, Overture meanings and reverse geocoding statements
//! that `pxl-geo` and `pxl-geo-bake` must agree on.

use pxl_spec::space::coord::zoom::{units, LEAF_ZOOM};

pub mod overture;
pub mod table;

/// How far past a leaf any bake reads, in units: a z13 tile, four terrain
/// leaves. A terrain leaf's reads stay within its 2-ring (pxl-geo-bake
/// `coverage.rs`); a division with no area is looked for this far from a
/// point, past the gap between such points nearly everywhere, by the search
/// and the bake alike (`overture::division`).
pub const BAKE_READ_DIST: f64 = units(LEAF_ZOOM - 2);
