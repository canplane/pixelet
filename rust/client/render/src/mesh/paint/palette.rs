// pxl-render/mesh/paint/palette.rs

//! Every color a cell can be, numbered once. Object colors follow the ground
//! layers, so one number reaches both.

use pxl_spec::asset::color;
use pxl_spec::asset::cube::cube_rgb;
use pxl_spec::asset::cube::{sentinel, Cube, ALL_CUBES};
use pxl_spec::asset::cube::{Ground, Object};
use pxl_spec::asset::ingested::CubeFace;
use pxl_spec::asset::{shipped_sentinel_mean, shipped_tile_mean};
use std::sync::LazyLock;

/// The sentinels the world draws, each a layer past the cubes': the bedrock.
pub const SENTINELS: [u8; 1] = [sentinel::unknown];

/// The texture array's layers, one a placeable cube in the catalog's order, so
/// a cube of a look is drawn as its ground is, then one a sentinel drawn.
pub const LAYERS: usize = Object::ALL.len() + Ground::ALL.len() + SENTINELS.len();

/// A drawn sentinel's layer, past every cube's.
pub fn sentinel_layer(s: u8) -> Option<u32> {
    let k = SENTINELS.iter().position(|&x| x == s)?;
    Some((ALL_CUBES.len() + k) as u32)
}

pub fn palette() -> impl Iterator<Item = Cube> {
    ALL_CUBES.iter().copied()
}

pub fn layer_idx(cube: Cube) -> u32 {
    palette()
        .position(|c| c == cube)
        .expect("a cube of the catalog") as u32
}

pub fn rgb255(rgb: u32) -> [u8; 3] {
    [
        ((rgb >> 16) & 255) as u8,
        ((rgb >> 8) & 255) as u8,
        (rgb & 255) as u8,
    ]
}

/// What a layer reads as once its tile is too small to see: the tile's mean,
/// or the swatch with no tile. One mean a layer, its top's, on every face, so
/// far off a cliff reads as its land use; Distant Horizons summarizes the same way.
pub static LAYER_MEAN: LazyLock<Vec<[u8; 3]>> = LazyLock::new(|| {
    palette()
        .map(|cube| {
            shipped_tile_mean(cube, CubeFace::Top)
                .unwrap_or_else(|| rgb255(cube_rgb(cube).unwrap_or(0)))
        })
        .chain(SENTINELS.iter().map(|&s| {
            shipped_sentinel_mean(s)
                .unwrap_or_else(|| color::sentinel(s).map_or([0; 3], |c| rgb255(c.rgb)))
        }))
        .collect()
});

pub fn layer_mean(layer: u32, rgb: [u8; 3]) -> [u8; 3] {
    LAYER_MEAN.get(layer as usize).copied().unwrap_or(rgb)
}
