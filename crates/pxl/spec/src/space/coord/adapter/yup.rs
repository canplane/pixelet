// pxl-spec/space/coord/adapter/yup.rs

//! Y-up <-> Z-up conversion on the CPU, for an asset or third-party boundary
//! only; the engine is Z-up and glTF models are turned on the GPU. Nothing
//! imports it, on purpose. Z-up (X, Y, Z) -> Y-up (X, Z, -Y).

use crate::space::coord::v3::V3;

/// Rotation for mounting Y-up XY-plane content into the Z-up world.
pub const YUP_TO_ZUP_ROTATION: [f64; 3] = [-core::f64::consts::PI / 2.0, 0.0, 0.0];

/// Converts a Z-up vector into Y-up coordinates.
pub fn yup<'a>(zup: &V3, out: &'a mut V3) -> &'a mut V3 {
    let zup_x = zup[0];
    let zup_y = zup[1];
    let zup_z = zup[2];
    out[0] = zup_x;
    out[1] = zup_z;
    out[2] = -zup_y;
    out
}

/// Converts a Y-up vector back into the native Z-up world basis.
pub fn zup<'a>(yup: &V3, out: &'a mut V3) -> &'a mut V3 {
    let yup_x = yup[0];
    let yup_y = yup[1];
    let yup_z = yup[2];
    out[0] = yup_x;
    out[1] = -yup_z;
    out[2] = yup_y;
    out
}
