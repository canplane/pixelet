// pxl-render/mesh/paint/object/adapter.rs

//! The materials object store values are drawn as, by the catalog.

use pxl_spec::cell::cube_types;
use pxl_spec::cell::object::{decor_ids, decor_num_id, placeable_cube_num_id, placeable_cubes};

use crate::look::color::mean_srgb;
use crate::mesh::paint::object::material::solid_material;
use crate::mesh::paint::palette::shader_meta::layer_mean;
use crate::mesh::word::{pack_material, unpack_material, NO_LAYER};

const AIR: u32 = cube_types::object::air as u32;

/// Decor renders as instanced geometry, not a cell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Untaught = 0,
    Solid = 1,
    Air = 2,
    Decor = 3,
}

pub struct Painter {
    kinds: Vec<Kind>,
    materials: Vec<u32>,
    layers: Vec<u8>,
}

impl Default for Painter {
    fn default() -> Self {
        Painter::new()
    }
}

impl Painter {
    pub fn new() -> Painter {
        let mut p = Painter {
            kinds: Vec::new(),
            materials: Vec::new(),
            layers: Vec::new(),
        };
        for &cube in placeable_cubes.iter() {
            let (material, layer) = solid_material(cube);
            p.set(placeable_cube_num_id(cube), Kind::Solid, material, layer);
        }
        p.set(AIR, Kind::Air, 0, NO_LAYER as u8);
        for &id in decor_ids.iter() {
            p.set(decor_num_id(id), Kind::Decor, 0, NO_LAYER as u8);
        }
        p
    }

    /// A state past a leaf's width is a catalog bug and is not set, rather than
    /// sizing the tables to it.
    fn set(&mut self, state: u32, kind: Kind, material: u32, layer: u8) {
        if state >= u16::MAX as u32 {
            return;
        }
        let i = state as usize;
        if i >= self.kinds.len() {
            self.kinds.resize(i + 1, Kind::Untaught);
            self.materials.resize(i + 1, 0);
            self.layers.resize(i + 1, NO_LAYER as u8);
        }
        self.kinds[i] = kind;
        self.materials[i] = material;
        self.layers[i] = layer;
    }

    /// None for untaught, air and decor.
    pub fn material_of(&self, state: u32) -> Option<(u32, u8)> {
        match self.kind(state) {
            Kind::Solid => Some((self.materials[state as usize], self.layers[state as usize])),
            _ => None,
        }
    }

    /// What a drawn cell of `of` shows over the children it folds, where they
    /// are not all `of`: the mean of what each solid one shows, the pack's mean
    /// where it is drawn, in `of`'s class and no layer. None where they are.
    pub fn mean_of(&self, of: u32, children: impl Iterator<Item = u32> + Clone, textured: bool) -> Option<(u32, u8)> {
        let (class, _) = unpack_material(self.material_of(of)?.0);
        if children.clone().all(|c| c == of) {
            return None;
        }
        let shown = children.filter_map(|c| self.material_of(c)).map(|(m, l)| {
            let rgb = unpack_material(m).1;
            if textured { layer_mean(u32::from(l), rgb) } else { rgb }
        });
        Some((pack_material(class, mean_srgb(shown)?), NO_LAYER as u8))
    }

    fn kind(&self, state: u32) -> Kind {
        self.kinds.get(state as usize).copied().unwrap_or(Kind::Untaught)
    }
}

#[cfg(test)]
#[path = "adapter.test.rs"]
mod test;
