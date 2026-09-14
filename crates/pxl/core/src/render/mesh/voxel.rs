// pxl_core/render/mesh/voxel.rs

//! What one voxel's bytes mean: its material class and colour packed into a
//! word, its texture layer, and the inset of a box that fills only part of
//! its cell. Pure encoding, shared by the volume that stores voxels, the
//! scan that reads them into runs and the mesher that draws the runs.

use crate::bake::terrain::js_round;

/// No texture-array layer applies; use the material's flat RGB.
pub const NO_LAYER: u32 = 255;

/// `MaterialClass`.
pub type MaterialClass = u32;

#[allow(non_snake_case)]
pub mod MaterialClasses {
    pub const AIR: u32 = 0;
    pub const LAND: u32 = 1;
    pub const WATER: u32 = 2;
    pub const TRANSPORT: u32 = 3;
    pub const STRUCTURE: u32 = 4;
    pub const OBJECT: u32 = 5;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoxelMaterial {
    pub material_class: MaterialClass,
    pub r: u32,
    pub g: u32,
    pub b: u32,
    pub layer_idx: u32,
}

pub fn pack_material(material: &VoxelMaterial) -> u32 {
    ((material.material_class & 255) << 24) | ((material.r & 255) << 16) | ((material.g & 255) << 8) | (material.b & 255)
}

pub fn unpack_material(packed: u32, layer_idx: u32) -> VoxelMaterial {
    VoxelMaterial {
        material_class: packed >> 24,
        r: (packed >> 16) & 255,
        g: (packed >> 8) & 255,
        b: packed & 255,
        layer_idx,
    }
}

pub fn pack_sub_cell(ox: u32, oy: u32, oz: u32, sz: u32) -> u32 {
    ox | (oy << 8) | (oz << 16) | (sz << 24)
}

pub const SUB_CELL_NONE: u32 = 0;
pub const SUB_CELL_SCALE: u32 = 256;

pub fn pack_sub_cell_fraction(fraction: f64) -> u32 {
    js_round(fraction * SUB_CELL_SCALE as f64).clamp(0.0, (SUB_CELL_SCALE - 1) as f64) as u32
}

pub fn unpack_sub_cell_fraction(packed: u32) -> f64 {
    packed as f64 / SUB_CELL_SCALE as f64
}

/// The largest packed value, read as exactly 1 rather than 255/256.
///
/// An offset of 1 is meaningless, so `unpack_sub_cell_fraction` never needs
/// to express it. A *size* of 1 is not: coarse LOD fits a box to what is
/// actually inside a cell, and a box that fills one axis while shrinking
/// another is the ordinary case, not an edge one. Reading the top value as
/// full costs a comparison and keeps 0 free as SUB_CELL_NONE.
pub const SUB_CELL_FULL: u32 = SUB_CELL_SCALE - 1;

pub fn unpack_sub_cell_size(packed: u32) -> f64 {
    if packed == SUB_CELL_FULL {
        1.0
    } else {
        packed as f64 / SUB_CELL_SCALE as f64
    }
}
