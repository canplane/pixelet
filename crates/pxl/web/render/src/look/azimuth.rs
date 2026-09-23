// pxl-render/look/azimuth.rs

//! The map's azimuth: the one turn about Z the view makes. The camera never
//! turns; the world turns under it (the kernel's world/frame/view.rs), and
//! everything drawn — the meshes, the sky, the stars, the clouds — is placed
//! in the map's own axes and turned by the view's matrix. What the CPU
//! measures in the turned world and has to place in the map's axes — the
//! planet's placement, for culling — is turned back first and forward after.
//!
//! Not in pxl-spec's v3: every operation there treats the three lanes alike,
//! and this one singles out Z.

use pxl_spec::space::v3::V3;

/// `v` turned about Z, counter-clockwise seen from above, by the angle whose
/// sine and cosine are `s` and `c`; turned back is `-s`. The angle comes as
/// its sine and cosine because the root frame's matrix already holds them,
/// and a caller turning many vectors takes them once.
pub fn turn<'a>(v: &V3, s: f64, c: f64, out: &'a mut V3) -> &'a mut V3 {
    let (x, y) = (v[0], v[1]);
    out[0] = x * c - y * s;
    out[1] = x * s + y * c;
    out[2] = v[2];
    out
}
