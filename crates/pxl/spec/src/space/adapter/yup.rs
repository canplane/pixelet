// pxl-spec/space/adapter/yup.rs

//! Y-up <-> Z-up conversion on the CPU, for a boundary where a Y-up basis
//! meets this project's.
//!
//! **Nothing imports this, and that is the expected state, not rot.** The
//! engine is Z-up throughout; ENU maps straight onto the render axes. The one
//! Y-up input it has, glTF models, is not converted here either:
//! crates/pxl/render/src/asset/gltf.rs keeps a model's own basis, and
//! crates/pxl/render/src/frame/shader/place.wgsl turns it to Z-up on the GPU as the model is placed.
//!
//! So: don't reach for these while working inside the world. Reach for them
//! only where Y-up coordinates have to be read on the CPU at an asset or
//! third-party boundary, and keep the conversion localized there rather than
//! letting Y-up coordinates travel inward.
//!
//! Basis transform:
//!   Z-up (X, Y, Z) -> Y-up (X, Z, -Y)

use crate::space::v3::V3;

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
