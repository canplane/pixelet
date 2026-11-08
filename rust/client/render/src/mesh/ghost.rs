// pxl-render/mesh/ghost.rs

//! The hover ghost's box, written as the five words the box shader (box.wgsl) unfolds into a
//! cube: its corner and width in u16 steps of its cell, its floor and height in u16 steps of
//! its mesh, its sRGB color, and its texture layer.

use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::round;

use crate::frame::shader::record::BOX_WORDS;
use crate::frame::shader::word::BOX_XY_STEPS;

/// A cell is `BOX_XY_STEPS` steps across. A box's z is held in its mesh's own steps, counted
/// from the mesh's floor.
const XY_STEPS: f64 = BOX_XY_STEPS as f64;
const Z_STEPS: f64 = u16::MAX as f64;

/// Where a mesh holding a box cornered at `min` stands, and the box's corner in it. The mesh
/// stands on the whole unit under the box, so a texture's repeat is read off the mesh's own
/// coordinates.
pub fn on_whole_units(min: V3) -> (V3, V3) {
    let origin = min.map(f64::floor);
    (
        origin,
        [min[0] - origin[0], min[1] - origin[1], min[2] - origin[2]],
    )
}

/// Writes the hover ghost's box at `off` in its mesh (`on_whole_units`), lit and tiled as a
/// placed cube is. Returns the mesh's z step, which its draw scales by.
pub fn write_lone_box(m: &mut [u32], off: V3, size: f64, rgb: [u8; 3], layer: u8) -> f64 {
    xpute_kit::ensure!(m.len() == BOX_WORDS as usize, ENOTRECOVERABLE);
    let z_step = (off[2] + size) / Z_STEPS;
    // Both ends are rounded, not the size, so stacked boxes meet at one step and never
    // overlap or leave a gap.
    let (z0, z1) = (steps(off[2], z_step), steps(off[2] + size, z_step));
    let xy = size / XY_STEPS;
    m[0] = steps(off[0], xy) | (steps(off[1], xy) << 16);
    m[1] = steps(size, xy) | (steps(size, xy) << 16);
    m[2] = z0 | ((z1 - z0) << 16);
    m[3] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
    m[4] = u32::from(layer);
    z_step
}

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}
