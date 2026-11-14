// pxl-spec/space/coord/earth.spec.rs
//
// GENERATED from spec/spec/space/earth.json (xpute-gen data.rs)
//
// The Earth as the map takes it: Web Mercator's sphere, which every meter the
// map measures is a meter of (pxl-spec space/coord/adapter/4326.rs).

// A table is generated whole, whether or not this crate calls an entry.
#![allow(dead_code, reason = "generated from the spec")]

/// WGS84's semi-major axis, the sphere EPSG:3857 projects.
pub const EARTH_RADIUS_M: f64 = 6378137.0;
