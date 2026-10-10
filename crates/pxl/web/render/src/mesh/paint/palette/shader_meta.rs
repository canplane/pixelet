// pxl-render/mesh/paint/palette/shader_meta.rs

//! The palette as tables, numbered as the texture pack's array layers, so paint
//! and arrays cannot disagree about what a type_id means.

use std::sync::LazyLock;

use pxl_spec::asset::texture::{shipped_tile_mean, CubeFace};
use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::cell::object::{block_cubes, ground_channel_types, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};

use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerMeta {
    pub ch_key: GroundChannelKey,
    pub type_id: u8,
    pub rgb: u32,
    pub layer_index: u32,
}

fn layer_meta() -> Vec<TerrainLayerMeta> {
    let mut out = Vec::new();
    for ch_key in GROUND_CH_KEYS {
        // AIR never resolves to a color here, so it is filtered out with every other
        // colorless type; VOID and UNKNOWN do resolve.
        for type_id in ground_channel_types(ch_key) {
            if let Some(swatch) = pxl_spec::cell::color::ground(ch_key, type_id) {
                let layer_index = out.len() as u32;
                out.push(TerrainLayerMeta {
                    ch_key,
                    type_id,
                    rgb: swatch.rgb,
                    layer_index,
                });
            }
        }
    }
    out
}

pub fn rgb255(rgb: u32) -> [u8; 3] {
    [((rgb >> 16) & 255) as u8, ((rgb >> 8) & 255) as u8, (rgb & 255) as u8]
}

pub static GROUND_LAYER_META: LazyLock<Vec<TerrainLayerMeta>> = LazyLock::new(layer_meta);

/// What a layer reads as once its tile is too small to see: the tile's mean,
/// or the swatch with no tile. One mean a layer, its top's, on every face, so
/// far off a cliff reads as its land use; Distant Horizons summarizes the same way.
pub static LAYER_MEAN: LazyLock<Vec<[u8; 3]>> = LazyLock::new(layer_means);

fn layer_means() -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = Vec::new();
    let mut put = |layer: u32, cube: Option<PlaceableCube>, rgb: u32| {
        let mean = cube.and_then(|cube| shipped_tile_mean(cube, CubeFace::Top)).unwrap_or_else(|| rgb255(rgb));
        let layer = layer as usize;
        if out.len() <= layer {
            out.resize(layer + 1, [0; 3]);
        }
        out[layer] = mean;
    };
    for m in GROUND_LAYER_META.iter() {
        put(m.layer_index, terrain_style_cube_for(m.ch_key, m.type_id as u32), m.rgb);
    }
    for block in block_cubes() {
        let cube = PlaceableCube::Block(block);
        if let (Some(layer), Some(rgb)) = (OBJECT_LAYER_IDX(block), placeable_cube_rgb(cube)) {
            put(layer, Some(cube), rgb);
        }
    }
    out
}

pub fn layer_mean(layer: u32, rgb: [u8; 3]) -> [u8; 3] {
    LAYER_MEAN.get(layer as usize).copied().unwrap_or(rgb)
}
