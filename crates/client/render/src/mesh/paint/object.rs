// pxl-render/mesh/paint/object.rs

//! The object store's records as voxels of a chunk: the materials its values
//! are drawn as, by the catalog.

use pxl_spec::block::cube;
use pxl_spec::block::object::{cube_id, decor_ids, decor_num_id, ALL_CUBES};

use crate::mesh::paint::palette::rgb255;
use crate::mesh::paint::palette::LAYER_IDX;
use crate::mesh::word::NO_LAYER;
use pxl_spec::block::object::{cube_rgb, Cube};
use std::sync::LazyLock;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const AIR: u32 = cube::object::air as u32;

/// Decor renders as instanced geometry, not a cell.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Untaught = 0,
    Solid = 1,
    Air = 2,
    Decor = 3,
}

/// The one table, fixed by the catalog.
pub static PAINTER: LazyLock<Painter> = LazyLock::new(Painter::new);

pub struct Painter {
    kinds: Vec<Kind>,
    swatches: Vec<[u8; 3]>,
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
            swatches: Vec::new(),
            layers: Vec::new(),
        };
        for &cube in ALL_CUBES.iter() {
            let (swatch, layer) = solid_look(cube);
            p.set(cube_id(cube), Kind::Solid, swatch, layer);
        }
        p.set(AIR, Kind::Air, [0; 3], NO_LAYER as u8);
        for &id in decor_ids.iter() {
            p.set(decor_num_id(id), Kind::Decor, [0; 3], NO_LAYER as u8);
        }
        p
    }

    /// A state past a leaf's width is a catalog bug and is not set, rather than
    /// sizing the tables to it.
    fn set(&mut self, state: u32, kind: Kind, swatch: [u8; 3], layer: u8) {
        if state >= u16::MAX as u32 {
            return;
        }
        let i = state as usize;
        if i >= self.kinds.len() {
            self.kinds.resize(i + 1, Kind::Untaught);
            self.swatches.resize(i + 1, [0; 3]);
            self.layers.resize(i + 1, NO_LAYER as u8);
        }
        self.kinds[i] = kind;
        self.swatches[i] = swatch;
        self.layers[i] = layer;
    }

    /// Its swatch and texture layer; none for untaught, air and decor.
    pub fn look_of(&self, state: u32) -> Option<([u8; 3], u8)> {
        match self.kind(state) {
            Kind::Solid => Some((self.swatches[state as usize], self.layers[state as usize])),
            _ => None,
        }
    }

    fn kind(&self, state: u32) -> Kind {
        self.kinds.get(state as usize).copied().unwrap_or(Kind::Untaught)
    }
}

/// Its swatch and texture layer: a cube of a look takes its ground's.
pub fn solid_look(cube: Cube) -> ([u8; 3], u8) {
    (rgb255(cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE)), LAYER_IDX(cube) as u8)
}

#[cfg(test)]
#[path = "object.test.rs"]
mod test;
