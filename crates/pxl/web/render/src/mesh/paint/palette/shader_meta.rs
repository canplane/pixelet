// pxl-render/mesh/paint/palette/shader_meta.rs

//! What a layer of the palette reads as from far off.

use std::sync::LazyLock;

use pxl_spec::asset::texture::{shipped_tile_mean, CubeFace};
use pxl_spec::cell::object::placeable_cube_rgb;

use crate::mesh::paint::palette::tex_array::palette;

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
