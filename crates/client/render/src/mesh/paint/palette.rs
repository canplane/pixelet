// pxl-render/mesh/paint/palette.rs

//! Every color a cell can be, numbered once. Object colors follow the ground
//! layers, so one number reaches both.

use pxl_spec::asset::ingested::CubeFace;
use pxl_spec::asset::shipped_tile_mean;
use pxl_spec::cell::cube_types::{Block, Ground};
use pxl_spec::cell::object::placeable_cube_rgb;
use pxl_spec::cell::object::{placeable_cubes, PlaceableCube};
use std::sync::LazyLock;

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

pub fn rgb255(rgb: u32) -> [u8; 3] {
    [((rgb >> 16) & 255) as u8, ((rgb >> 8) & 255) as u8, (rgb & 255) as u8]
}

/// What a layer reads as once its tile is too small to see: the tile's mean,
/// or the swatch with no tile. One mean a layer, its top's, on every face, so
/// far off a cliff reads as its land use; Distant Horizons summarizes the same way.
pub static LAYER_MEAN: LazyLock<Vec<[u8; 3]>> = LazyLock::new(|| {
    palette()
        .map(|cube| shipped_tile_mean(cube, CubeFace::Top).unwrap_or_else(|| rgb255(placeable_cube_rgb(cube).unwrap_or(0))))
        .collect()
});

pub fn layer_mean(layer: u32, rgb: [u8; 3]) -> [u8; 3] {
    LAYER_MEAN.get(layer as usize).copied().unwrap_or(rgb)
}
