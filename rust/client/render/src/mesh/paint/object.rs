// pxl-render/mesh/paint/object.rs

//! The object store's records as blocks: the materials their values are
//! drawn as, by the catalog.

use pxl_spec::asset::cube;
use pxl_spec::asset::cube::{cube_id, ALL_CUBES};
use pxl_spec::asset::{decor_ids, decor_num_id};

use crate::mesh::paint::palette::rgb255;
use crate::mesh::paint::palette::LAYER_IDX;
use pxl_spec::asset::cube::{cube_rgb, Cube};
use std::sync::LazyLock;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

const AIR: u32 = cube::object::air as u32;

/// The one table, fixed by the catalog.
pub static PAINTER: LazyLock<Painter> = LazyLock::new(Painter::new);

/// Every state the catalog names, a cube's look its own; air and decor,
/// which no cell is drawn as, none.
pub struct Painter {
    looks: Vec<Option<([u8; 3], u8)>>,
}

impl Default for Painter {
    fn default() -> Self {
        Painter::new()
    }
}

impl Painter {
    pub fn new() -> Painter {
        // A state past a leaf's width is a catalog bug and is not named,
        // rather than sizing the table to it.
        let named = ALL_CUBES
            .iter()
            .map(|&c| cube_id(c))
            .chain([AIR])
            .chain(decor_ids.iter().map(|&id| decor_num_id(id)));
        let states = named
            .filter(|&s| s < u16::MAX as u32)
            .max()
            .map_or(0, |s| s as usize + 1);
        let mut looks = vec![None; states];
        for &cube in ALL_CUBES.iter() {
            if let Some(l) = looks.get_mut(cube_id(cube) as usize) {
                *l = Some(solid_look(cube));
            }
        }
        Painter { looks }
    }

    /// The states the catalog names, every one under it.
    pub fn states(&self) -> usize {
        self.looks.len()
    }

    /// Its swatch and texture layer; none for air, decor and what is not named.
    pub fn look_of(&self, state: u32) -> Option<([u8; 3], u8)> {
        self.looks.get(state as usize).copied().flatten()
    }
}

/// Its swatch and texture layer: a cube of a look takes its ground's.
pub fn solid_look(cube: Cube) -> ([u8; 3], u8) {
    (
        rgb255(cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE)),
        LAYER_IDX(cube) as u8,
    )
}
