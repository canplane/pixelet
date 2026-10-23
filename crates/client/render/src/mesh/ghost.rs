// pxl-render/mesh/ghost.rs

//! The hover ghost's box, in the six words the box shader (box.wgsl) unfolds
//! into a cube: corner and width in u16 steps of its cell, floor and height
//! in u16 steps of its mesh, sRGB color, four layer taps, face mask.

use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::round;

use crate::mesh::word::taps_of;
use pxl_spec::block::mask::{ALL_CREASES, ALL_FACES};

pub const BOX_WORDS: u32 = 6;

/// A cell is 256 steps; a box's z is held in its mesh's own steps from the
/// mesh's floor, since a block's z is the world's.
const XY_STEPS: f64 = 256.0;
const Z_STEPS: f64 = 65535.0;

/// Every lateral edge a crease, so each shades as a silhouette.
const LONE_BOX_FACES: u32 = ALL_FACES | ALL_CREASES;

/// Where a mesh holding a box cornered at `min` stands, and the box in it: on
/// the whole unit under it, so a texture's repeat is read off the mesh's own
/// coordinates.
pub fn on_whole_units(min: V3) -> (V3, V3) {
    let origin = min.map(f64::floor);
    (origin, [min[0] - origin[0], min[1] - origin[1], min[2] - origin[2]])
}

/// The hover ghost's box at `offset` in its mesh (`on_whole_units`), lit and
/// tiled as a placed cube is; the mesh's z step, which its draw scales by.
pub fn write_lone_box(m: &mut [u32], offset: V3, size: f64, rgb: [u8; 3], layer: u8) -> f64 {
    xpute_kit::ensure!(m.len() == BOX_WORDS as usize, ENOTRECOVERABLE);
    let z_step = (offset[2] + size) / Z_STEPS;
    // Both ends rounded, not the size: boxes stacked meet at one step, never
    // a step over or apart.
    let (z0, z1) = (steps(offset[2], z_step), steps(offset[2] + size, z_step));
    let xy = size / XY_STEPS;
    m[0] = steps(offset[0], xy) | (steps(offset[1], xy) << 16);
    m[1] = steps(size, xy) | (steps(size, xy) << 16);
    m[2] = z0 | ((z1 - z0) << 16);
    m[3] = u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 0]);
    m[4] = taps_of(layer);
    m[5] = LONE_BOX_FACES;
    z_step
}

fn steps(v: f64, step: f64) -> u32 {
    round(v / step).clamp(0.0, u16::MAX as f64) as u32
}
