// pxl-render/mesh/paint/palette/shader_meta.rs

//! The palette as tables: every (channel, type) the cube catalog colors,
//! numbered as the layers of the texture pack's arrays (layers builds the
//! arrays in this order); the placed cubes' colors are numbered after them
//! (tex_array.rs).
//! The terrain's materials (terrain/adapter.rs `MATERIALS`) and a placed
//! cube's (object/material.rs) are read from here, so the paint and the
//! arrays cannot disagree about what a type_id means.

use std::sync::LazyLock;

use pxl_spec::asset::texture::{shipped_tile_mean, CubeFace};
use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::cell::object::{block_cubes, ground_channel_types, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};

use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// A colored type of a channel, and the array layer it is.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerMeta {
    pub ch_key: GroundChannelKey,
    pub type_id: u8,
    /// Packed `0xRRGGBB`, as the catalog gives it.
    pub rgb: u32,
    pub layer_index: u32,
}

/// One array layer per (channel, type) pair, in the catalog's order.
fn layer_meta() -> Vec<TerrainLayerMeta> {
    let mut out = Vec::new();
    for ch_key in GROUND_CH_KEYS {
        // A channel carries only its own real types — VOID/AIR/UNKNOWN live in
        // cube_types::common instead — so they are walked first for this
        // iteration, not merged into the data: AIR never resolves to a color
        // for any of these channels, so it's filtered out below as any other
        // type with no real color is; VOID and UNKNOWN do.
        for type_id in ground_channel_types(ch_key) {
            // Every swatch the catalog holds is a color: the generator packs
            // it from the JSON and refuses anything that is not #RRGGBB, so
            // there is nothing left here to validate.
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

/// A packed `0xRRGGBB` as its three bytes.
pub fn rgb255(rgb: u32) -> [u8; 3] {
    [((rgb >> 16) & 255) as u8, ((rgb >> 8) & 255) as u8, (rgb & 255) as u8]
}

/// Static — cube_colors/cube_types never change at runtime, so this is built
/// once rather than per-chunk.
pub static GROUND_LAYER_META: LazyLock<Vec<TerrainLayerMeta>> = LazyLock::new(layer_meta);

/// What each array layer reads as once its tile is too small to see: the
/// tile's own mean color (asset/ingested.spec.rs's `means`), and the
/// catalog's swatch for a type the pack has no tile for.
///
/// A summary of a patch of ground is drawn in this and not in the swatch.
/// The swatch is the color a type is filed under — the color a legend
/// would use — while the tile is what the ground actually shows, and the two
/// part company wherever a tile is painted rather than flat.
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

/// The color a run of layer `layer` is summarized in: the layer's mean where
/// the pack has one, and `rgb` — the run's own — where it has none.
pub fn layer_mean(layer: u32, rgb: [u8; 3]) -> [u8; 3] {
    LAYER_MEAN.get(layer as usize).copied().unwrap_or(rgb)
}

/// A channel's colors by type, as flat u8-indexed arrays rather than a map:
/// the per-cell decode was dominated by map lookups when it was one. `has` is
/// a separate 0/1 flag array (not a sentinel color) because [0,0,0] is
/// itself sometimes a real mapped color.
#[derive(Clone, Debug)]
pub struct FlatChannelLookup {
    pub rgb: [u8; 256 * 3],
    pub has: [u8; 256],
}

/// By channel, in GROUND_CH_KEYS' order; None for a channel with no color.
pub type FlatGroundColorLookup = [Option<FlatChannelLookup>; 4];

fn channel_slot(ch: GroundChannelKey) -> usize {
    GROUND_CH_KEYS.iter().position(|k| *k == ch).or_bug(Errno::ENOTRECOVERABLE)
}

fn build_flat_color_lookup(meta: &[TerrainLayerMeta]) -> FlatGroundColorLookup {
    let mut out: FlatGroundColorLookup = [None, None, None, None];
    for m in meta {
        let entry = out[channel_slot(m.ch_key)].get_or_insert(FlatChannelLookup { rgb: [0; 768], has: [0; 256] });
        let [r, g, b] = rgb255(m.rgb);
        let t = m.type_id as usize;
        entry.rgb[t * 3] = r;
        entry.rgb[t * 3 + 1] = g;
        entry.rgb[t * 3 + 2] = b;
        entry.has[t] = 1;
    }
    out
}

pub static GROUND_COLOR_LOOKUP_FLAT: LazyLock<FlatGroundColorLookup> = LazyLock::new(|| build_flat_color_lookup(&GROUND_LAYER_META));

/// `(ch_key, type_id) -> layer_index`, direct u8-indexed for the same reason
/// GROUND_COLOR_LOOKUP_FLAT is. `has` mirrors its reasoning: layer_index 0 is
/// itself a real, valid slot.
#[derive(Clone, Debug)]
pub struct FlatLayerIndex {
    pub index: [u8; 256],
    pub has: [u8; 256],
}

pub type FlatLayerIndexLookup = [Option<FlatLayerIndex>; 4];

fn build_flat_layer_index_lookup(meta: &[TerrainLayerMeta]) -> FlatLayerIndexLookup {
    let mut out: FlatLayerIndexLookup = [None, None, None, None];
    for m in meta {
        let entry = out[channel_slot(m.ch_key)].get_or_insert(FlatLayerIndex { index: [0; 256], has: [0; 256] });
        entry.index[m.type_id as usize] = m.layer_index as u8;
        entry.has[m.type_id as usize] = 1;
    }
    out
}

pub static GROUND_LAYER_INDEX_LOOKUP_FLAT: LazyLock<FlatLayerIndexLookup> = LazyLock::new(|| build_flat_layer_index_lookup(&GROUND_LAYER_META));

/// A channel's color lookup.
pub fn ground_color_lookup(ch: GroundChannelKey) -> Option<&'static FlatChannelLookup> {
    GROUND_COLOR_LOOKUP_FLAT[channel_slot(ch)].as_ref()
}

/// A channel's layer lookup.
pub fn ground_layer_index_lookup(ch: GroundChannelKey) -> Option<&'static FlatLayerIndex> {
    GROUND_LAYER_INDEX_LOOKUP_FLAT[channel_slot(ch)].as_ref()
}
