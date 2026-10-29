// pxl-render/mesh/ghost.rs

//! The hover ghost's box, in the five words the box shader (box.wgsl) unfolds
//! into a cube: corner and width in u16 steps of its cell, floor and height
//! in u16 steps of its mesh, sRGB color, its layer.

use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::round;

use crate::frame::shader::record::BOX_WORDS;
use crate::frame::shader::word::BOX_XY_STEPS;

/// A cell is `BOX_XY_STEPS`; a box's z is held in its mesh's own steps from
/// the mesh's floor, since a cell's z is the world's.
const XY_STEPS: f64 = BOX_XY_STEPS as f64;
const Z_STEPS: f64 = u16::MAX as f64;

/// Where a mesh holding a box cornered at `min` stands, and the box in it: on
/// the whole unit under it, so a texture's repeat is read off the mesh's own
/// coordinates.
pub fn on_whole_units(min: V3) -> (V3, V3) {
    let origin = min.map(f64::floor);
    (
        origin,
        [min[0] - origin[0], min[1] - origin[1], min[2] - origin[2]],
    )
}

/// The hover ghost's box at `offset` in its mesh (`on_whole_units`), lit and
/// tiled as a placed cube is; the mesh's z step, which its draw scales by.
pub fn write_lone_box(m: &mut [u32], off: V3, sz: f64, rgb: [u8; 3], layer: u8) -> f64 {
    xpute_kit::ensure!(m.len() == BOX_WORDS as usize, ENOTRECOVERABLE);
    let z_step = (off[2] + sz) / Z_STEPS;
    // Both ends rounded, not the size: boxes stacked meet at one step, never
    // a step over or apart.
    let (z0, z1) = (steps(off[2], z_step), steps(off[2] + sz, z_step));
    let xy = sz / XY_STEPS;
    m[0] = steps(off[0], xy) | (steps(off[1], xy) << 16);
    m[1] = steps(sz, xy) | (steps(sz, xy) << 16);
    m[2] = z0 | ((z1 - z0) << 16);
    m[3] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
    m[4] = u32::from(layer);
    z_step
}

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}
