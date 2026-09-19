// pxl-render/mesh/paint/palette/shader_meta.rs

//! The palette as tables: every (channel, type) the cube catalogue colours,
//! numbered as the layers of the texture pack's arrays (layers builds the
//! arrays in this order); the placed cubes' colours are numbered after them
//! (tex_array.rs).
//! What the kernel is taught for the terrain (terrain/teach) and the objects
//! (object/teach) reads from here, so the kernel and the arrays cannot
//! disagree about what a type_id means.

use std::sync::LazyLock;

use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::cell::object::{ground_channel_colors, ground_channel_types};

/// A coloured type of a channel, and the array layer it is.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerMeta {
    pub ch_key: GroundChannelKey,
    pub type_id: u8,
    /// Packed `0xRRGGBB`, as the catalog gives it.
    pub rgb: u32,
    pub layer_index: u32,
}

/// One array layer per (channel, type) pair, in the catalogue's order.
fn layer_meta() -> Vec<TerrainLayerMeta> {
    let mut out = Vec::new();
    for ch_key in GROUND_CH_KEYS {
        // A channel carries only its own real types — VOID/AIR/UNKNOWN live in
        // cube_types::common instead — so they are walked first for this
        // iteration, not merged into the data: AIR never resolves to a colour
        // for any of these channels, so it's filtered out below as any other
        // type with no real colour is; VOID and UNKNOWN do.
        let Some(colors) = ground_channel_colors(ch_key) else { continue };
        for type_id in ground_channel_types(ch_key) {
            // Every swatch the catalog holds is a colour: the generator packs
            // it from the JSON and refuses anything that is not #RRGGBB, so
            // there is nothing left here to validate.
            if let Some(Some(swatch)) = colors.get(&(type_id as u32)) {
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

/// A channel's colours by type, as flat u8-indexed arrays rather than a map:
/// the per-cell decode was dominated by map lookups when it was one. `has` is
/// a separate 0/1 flag array (not a sentinel colour) because [0,0,0] is
/// itself sometimes a real mapped colour.
#[derive(Clone, Debug)]
pub struct FlatChannelLookup {
    pub rgb: [u8; 256 * 3],
    pub has: [u8; 256],
}

/// By channel, in GROUND_CH_KEYS' order; None for a channel with no colour.
pub type FlatGroundColorLookup = [Option<FlatChannelLookup>; 4];

fn channel_slot(ch: GroundChannelKey) -> usize {
    GROUND_CH_KEYS.iter().position(|k| *k == ch).expect("a ground channel")
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

/// A channel's colour lookup.
pub fn ground_color_lookup(ch: GroundChannelKey) -> Option<&'static FlatChannelLookup> {
    GROUND_COLOR_LOOKUP_FLAT[channel_slot(ch)].as_ref()
}

/// A channel's layer lookup.
pub fn ground_layer_index_lookup(ch: GroundChannelKey) -> Option<&'static FlatLayerIndex> {
    GROUND_LAYER_INDEX_LOOKUP_FLAT[channel_slot(ch)].as_ref()
}
