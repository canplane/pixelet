// pxl-render/mesh/paint/object.rs

//! The materials the object store's values are drawn as, as the catalog defines them.

use pxl_spec::asset::cube::{cube_rgb, ALL_CUBES};
use pxl_spec::asset::decor::DECORS;
use pxl_spec::asset::AssetId;
use pxl_spec::value::AIR;

use crate::mesh::paint::palette::layer_idx;
use crate::mesh::paint::palette::rgb255;
use std::sync::LazyLock;
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// The one painter, built once from the catalog.
pub static PAINTER: LazyLock<Painter> = LazyLock::new(Painter::new);

/// A look for every state the catalog names. A cube has its own look. Air and decor have
/// none, as no cell is drawn as them.
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
        // A state past a leaf's width is a catalog bug and is not named.
        let named = ALL_CUBES
            .iter()
            .copied()
            .chain([AIR])
            .chain(DECORS.iter().map(|d| d.id));
        let states = named
            .filter(|&s| s < u16::MAX as u32)
            .max()
            .map_or(0, |s| s as usize + 1);
        let mut looks = vec![None; states];
        for &cube in ALL_CUBES.iter() {
            if let Some(l) = looks.get_mut(cube as usize) {
                *l = Some(solid_look(cube));
            }
        }
        Painter { looks }
    }

    /// The number of states. Every state the catalog names is below it.
    pub fn states(&self) -> usize {
        self.looks.len()
    }

    /// The swatch and texture layer of `state`; none for air, decor and a state the catalog
    /// does not name.
    pub fn look_of(&self, state: u32) -> Option<([u8; 3], u8)> {
        self.looks.get(state as usize).copied().flatten()
    }
}

/// The swatch and texture layer of `cube`. A cube of a look takes its ground's.
pub fn solid_look(cube: AssetId) -> ([u8; 3], u8) {
    (
        rgb255(cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE)),
        layer_idx(cube) as u8,
    )
}
