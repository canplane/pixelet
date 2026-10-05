// pxl-render/mesh/paint/palette/tex_array.rs

use pxl_spec::cell::cube::cube_types::{Block, Ground};
use pxl_spec::cell::object::{placeable_cubes, PlaceableCube};

/// The texture array's layers, one a placeable cube in the catalog's order, so
/// a cube of a look is drawn as its ground is.
pub const LAYERS: usize = Block::ALL.len() + Ground::ALL.len();

pub fn palette() -> impl Iterator<Item = PlaceableCube> {
    placeable_cubes.iter().copied()
}

#[allow(non_snake_case)]
pub fn LAYER_IDX(cube: PlaceableCube) -> u32 {
    palette().position(|c| c == cube).expect("a cube of the catalog") as u32
}
