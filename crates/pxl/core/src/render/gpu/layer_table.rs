// pxl_core/render/gpu/layer_table.rs

//! The palette as tables: every (channel, type) the cube catalogue colours,
//! numbered as the layers of the texture pack's arrays (layers builds the
//! arrays in this order), and the placed cubes' colours numbered after them.
//! What the kernel is taught for the terrain (terrain/teach) and the objects
//! (object/teach) reads from here, so the kernel and the arrays cannot
//! disagree about what a type_id means.

use std::sync::LazyLock;

use pxl_shared::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_shared::spec::cell::{cube_colors, cube_ids, cube_types};

/// A coloured type of a channel, and the array layer it is.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerMeta {
    pub ch_key: GroundChannelKey,
    pub type_key: &'static str,
    pub type_id: u8,
    pub hex_color: &'static str,
    pub layer_index: u32,
}

fn is_valid_hex(hex: &str) -> bool {
    let Some(digits) = hex.strip_prefix('#') else { return false };
    matches!(digits.len(), 3 | 4 | 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
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
        let channel = cube_types::CHANNELS.iter().find(|(name, _)| *name == ch_key.as_str()).map_or(&[][..], |(_, types)| types);
        let Some(colors) = cube_colors.get(ch_key.as_str()) else { continue };
        for &(type_key, type_id) in cube_types::common::ENTRIES.iter().chain(channel.iter()) {
            if let Some(Some(hex_color)) = colors.get(&(type_id as u32)) {
                if is_valid_hex(hex_color) {
                    let layer_index = out.len() as u32;
                    out.push(TerrainLayerMeta {
                        ch_key,
                        type_key,
                        type_id,
                        hex_color,
                        layer_index,
                    });
                }
            }
        }
    }
    out
}

pub fn hex_to_rgb255(hex: &str) -> [u8; 3] {
    let h: String = if hex.len() <= 5 { hex[1..4].chars().flat_map(|c| [c, c]).collect() } else { hex[1..7].to_string() };
    let num = u32::from_str_radix(&h, 16).unwrap_or(0);
    [((num >> 16) & 255) as u8, ((num >> 8) & 255) as u8, (num & 255) as u8]
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
        let [r, g, b] = hex_to_rgb255(m.hex_color);
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

/// Object colors, numbered starting right after every ground layer — layers
/// builds the array in this order, and object/material.rs reads this to
/// resolve a placed cube's layer_idx the same way the terrain reads
/// GROUND_LAYER_INDEX_LOOKUP_FLAT. A numbering, not a texture: what a placed
/// cube's layer is must be answerable where no canvas exists.
///
/// cube_ids is only ever the original 15 flat colors — the terrain-styled
/// catalog is a separate numbering, deliberately never mixed into this one:
/// those ids are meant to look identical to an existing ground layer, not get
/// a new one of their own.
#[allow(non_snake_case)]
pub fn OBJECT_LAYER_IDX(color_id: &str) -> Option<u32> {
    cube_ids.iter().position(|id| *id == color_id).map(|i| (GROUND_LAYER_META.len() + i) as u32)
}
