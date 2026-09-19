// pxl-kernel/layer/terrain/teach.rs

//! What the terrain paint is taught about the materials
//! (crates/pxl/web/render/src/mesh/paint/terrain/adapter.rs): the channels' classes, the bare base,
//! and each type's colour and layer from the palette's tables
//! (crates/pxl/web/render/src/mesh/paint/palette/shader_meta.rs).

use pxl_render::mesh::paint::palette::shader_meta::{ground_color_lookup, ground_layer_index_lookup};
use pxl_render::mesh::volume::{MaterialClass, MaterialClasses, NO_LAYER};
use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::cell::cube_types;

use crate::kernel::kernel;

pub fn ch_to_class(ch: GroundChannelKey) -> MaterialClass {
    match ch {
        GroundChannelKey::LAND => MaterialClasses::LAND,
        GroundChannelKey::WATER => MaterialClasses::WATER,
        GroundChannelKey::STRUCTURE => MaterialClasses::STRUCTURE,
        GroundChannelKey::TRANSPORT => MaterialClasses::TRANSPORT,
    }
}

/// The bare-ground base every cell starts from, before its channels.
const BASE_TYPE: usize = cube_types::land::bare as usize;

pub fn base_rgb() -> [u8; 3] {
    match ground_color_lookup(GroundChannelKey::LAND) {
        Some(flat) if flat.has[BASE_TYPE] != 0 => [flat.rgb[BASE_TYPE * 3], flat.rgb[BASE_TYPE * 3 + 1], flat.rgb[BASE_TYPE * 3 + 2]],
        _ => [0; 3],
    }
}

pub fn base_layer_idx() -> u32 {
    match ground_layer_index_lookup(GroundChannelKey::LAND) {
        Some(layer) if layer.has[BASE_TYPE] != 0 => layer.index[BASE_TYPE] as u32,
        _ => NO_LAYER,
    }
}

/// Hands the paint the tables a cell's material is read from, whole: the
/// base, and each channel's class and every type's colour and layer, in
/// GROUND_CH_KEYS' order. Once, before any page is told.
pub fn teach_terrain() {
    let t = &mut kernel().terrain;
    t.base_rgb = base_rgb();
    t.base_layer = base_layer_idx() as u8;
    t.base_class = MaterialClasses::LAND as u8;
    for (dst, ch) in t.channels.iter_mut().zip(GROUND_CH_KEYS) {
        dst.class = ch_to_class(ch) as u8;
        let color = ground_color_lookup(ch);
        let layer = ground_layer_index_lookup(ch);
        for k in 0..256 {
            dst.rgb[k] = color.map_or([0; 3], |c| [c.rgb[k * 3], c.rgb[k * 3 + 1], c.rgb[k * 3 + 2]]);
            dst.has_color[k] = color.is_some_and(|c| c.has[k] != 0);
            dst.layer[k] = layer.map_or(0, |l| l.index[k]);
            dst.has_layer[k] = layer.is_some_and(|l| l.has[k] != 0);
        }
    }
}
