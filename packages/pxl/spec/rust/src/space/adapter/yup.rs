// @pxl/spec/space/adapter/yup.ts
//
// Legacy Y-up <-> Z-up conversion helpers.
//
// Current world/camera runtime is native Z-up.
// This module is for legacy Three.js Y-up assets and compatibility transforms.
//
// Basis transform:
//   Z-up (X, Y, Z) -> Y-up Three-style (X, Z, -Y)

/// Legacy rotation for mounting Y-up XY-plane content into the Z-up world.
pub const YUP_TO_ZUP_ROTATION: [f64; 3] = [-std::f64::consts::FRAC_PI_2, 0.0, 0.0];

/// Converts a Z-up vector into legacy Y-up Three-style coordinates.
/// out = (x, z, -y)
pub fn yup(zup: [f64; 3]) -> [f64; 3] {
    [zup[0], zup[2], -zup[1]]
}

/// Converts a legacy Y-up vector back into the native Z-up world basis.
/// out = (x, -z, y)
pub fn zup(yup: [f64; 3]) -> [f64; 3] {
    [yup[0], -yup[2], yup[1]]
}
