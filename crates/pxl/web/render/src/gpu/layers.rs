// pxl-render/gpu/layers.rs

//! The texture pack as one 2D array texture on the device, and the table a
//! layer's two faces are found in it through.
//!
//! A slot of the array is a distinct tile of the pack, not a layer: a side
//! with no tile of its own wears the top's and shares its slot, and a layer
//! the pack ships nothing for takes none at all — it reads as the box's own
//! colour, which the shader already has. The table turns the layer index the
//! mesher bakes into a box's taps (mesh/paint/palette/shader_meta.rs) into
//! the slot its top and its side are in. Two arrays of a layer each, one the
//! copy of the other for most of them, was half again the memory and twice
//! the samples a fragment took.
//!
//! The array keeps its mip chain: a tile a cell minifies to a few texels at
//! the far tiers, and without the chain every camera movement resamples a
//! different texel and the ground crawls. WebGPU has no generateMipmap, so the
//! chain is drawn on the device, a level from the one above (Mips).

use pxl_spec::asset::texture::{shipped_texture_tile, CubeFace, TileOrigin, SHIPPED_SHEET};
use pxl_spec::cell::object::{block_cubes, placeable_cube_rgb, terrain_style_cube_for, PlaceableCube};

use crate::frame::shader::record::PIPELINE_MIPS;
use crate::gpu::api::{buffer_usage, bytes, object, texture_usage, Gpu};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::paint::Paint;
use crate::gpu::pipeline::Pipelines;
use crate::mesh::paint::palette::shader_meta::GROUND_LAYER_META;
use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;

/// The tile edge in texels (ingest-asset/pack.rs's TILE_SIZE).
pub const LAYER_TEX_SZ: u32 = 256;

/// Levels a texture of `width` by `height` has, down to one texel.
pub fn mip_levels(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

pub struct Mips {
    pipeline: u32,
    layout: u32,
    sampler: u32,
}

impl Mips {
    /// The builder for textures of `format`: a pipeline draws into the
    /// format it was built for, so a chain of another needs its own.
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, format: u32) -> Mips {
        let built = pipelines.build(g, PIPELINE_MIPS, format);
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::NEAREST, address::CLAMP, compare::NONE, 1);
        Mips {
            pipeline: built.pipeline,
            layout: built.layouts[0],
            sampler,
        }
    }

    /// Draws every level of `texture` below the first, for `layers` array
    /// layers from `first_layer`, each box-filtered from the one above.
    pub fn build(&self, g: &Gpu, texture: u32, levels: u32, first_layer: u32, layers: u32) {
        let encoder = object();
        g.encoder(encoder);
        for layer in first_layer..first_layer + layers {
            for level in 1..levels {
                let above = object();
                g.view(above, texture, dimension::D2, level - 1, 1, layer, 1);
                let target = object();
                g.view(target, texture, dimension::D2, level, 1, layer, 1);
                let group = object();
                g.bind_group(group, self.layout, &[0, resource::VIEW, above, 0, 0, 1, resource::SAMPLER, self.sampler, 0, 0]);
                let pass = object();
                g.pass(pass, encoder, &crate::gpu::api::pass_desc(target, load::CLEAR, 0, load::CLEAR, 0.0, crate::gpu::api::Stamp::UNTIMED));
                g.set_pipeline(pass, self.pipeline);
                g.set_bind_group(pass, 0, group, &[]);
                g.draw(pass, 3, 1, 0, 0);
                g.end(pass);
                // The encoder keeps what it recorded: a view, a group or a pass
                // let go now is gone from the table, not from the commands.
                for id in [pass, group, target, above] {
                    crate::gpu::api::release(id);
                }
            }
        }
        g.submit(encoder);
        crate::gpu::api::release(encoder);
    }
}

/// Every layer: the cube it is, where one stands for it, its index, and its
/// colour. A ground type no cube stands in for (common's) has no tile.
fn each_layer(mut f: impl FnMut(Option<PlaceableCube>, u32, u32)) {
    for m in GROUND_LAYER_META.iter() {
        f(terrain_style_cube_for(m.ch_key, m.type_id as u32), m.layer_index, m.rgb);
    }
    for block in block_cubes() {
        let cube = PlaceableCube::Block(block);
        if let (Some(layer), Some(rgb)) = (OBJECT_LAYER_IDX(block), placeable_cube_rgb(cube)) {
            f(Some(cube), layer, rgb);
        }
    }
}

/// A face with no tile of its own in the pack: the box's own colour is what
/// it reads as, and no slot of the array is spent waiting for one.
const NO_SLOT: u32 = 0xffff;

/// Where a layer's two faces are drawn in the array, packed as the shader
/// reads them: the top in the low half, the side in the high.
fn slot_word(top: u32, side: u32) -> u32 {
    top | (side << 16)
}

/// The array the pack needs, and the table that finds a face in it: one slot
/// a distinct tile, so a side that wears the top's shares its slot and a
/// layer the pack ships nothing for takes none at all.
fn plan() -> (Vec<TileOrigin>, Vec<u32>) {
    fn slot_of(slots: &mut Vec<TileOrigin>, tile: Option<TileOrigin>) -> u32 {
        let Some(tile) = tile else { return NO_SLOT };
        match slots.iter().position(|s| *s == tile) {
            Some(i) => i as u32,
            None => {
                slots.push(tile);
                (slots.len() - 1) as u32
            }
        }
    }
    let mut slots: Vec<TileOrigin> = Vec::new();
    let mut table: Vec<u32> = Vec::new();
    each_layer(|cube, layer, _| {
        let top = cube.and_then(|cube| shipped_texture_tile(cube, CubeFace::Top));
        // A side with no tile of its own wears the top's, which is the same
        // tile and so the same slot.
        let side = cube.and_then(|cube| shipped_texture_tile(cube, CubeFace::Side)).or(top);
        let word = slot_word(slot_of(&mut slots, top), slot_of(&mut slots, side));
        if table.len() <= layer as usize {
            table.resize(layer as usize + 1, slot_word(NO_SLOT, NO_SLOT));
        }
        table[layer as usize] = word;
    });
    (slots, table)
}

pub struct Layers {
    tex: u32,
    pub view: u32,
    /// Where each layer's faces are in the array, as the shader reads it.
    pub table: u32,
    pub sampler: u32,
    pub bytes: u32,
    slots: u32,
    levels: u32,
    /// The pack's sheet drawn into the array.
    loaded: bool,
}

impl Layers {
    /// The array the pack's tiles go in, and the table that finds them.
    ///
    /// Nothing is drawn into either until the sheet arrives: a layer's flat
    /// colour is the box's own colour, which the shader already has, so a
    /// slot filled with it would be a mip chain built to say what the vertex
    /// says. Until then the table sends every face to NO_SLOT.
    pub fn new(g: &Gpu) -> Layers {
        let (tiles, _) = plan();
        let slots = tiles.len() as u32;
        let levels = mip_levels(LAYER_TEX_SZ, LAYER_TEX_SZ);
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        let tex = object();
        g.texture(tex, LAYER_TEX_SZ, LAYER_TEX_SZ, slots, format::RGBA8_SRGB, levels, usage);
        let view = object();
        g.view(view, tex, dimension::D2_ARRAY, 0, levels, 0, slots);
        // Anisotropic at the four three's arrays were filtered at: the ground
        // is mostly seen at a grazing angle, where an isotropic mip blurs the
        // tile along the view.
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::REPEAT, compare::NONE, 4);
        let table = object();
        let blank: Vec<u32> = vec![slot_word(NO_SLOT, NO_SLOT); table_len()];
        g.buffer(table, (blank.len() * 4) as u32, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(table, 0, bytes(&blank));
        // A chain is the sum of its levels' texels: under four thirds of the first.
        let bytes = (0..levels).map(|level| (LAYER_TEX_SZ >> level).pow(2) * 4 * slots).sum();
        Layers {
            tex,
            view,
            table,
            sampler,
            bytes,
            slots,
            levels,
            loaded: false,
        }
    }

    /// Draws the pack's sheet into the array once it has arrived, and opens
    /// the table onto it: `sheet` asks for its image, until it is drawn.
    pub fn step(&mut self, g: &Gpu, mips: &Mips, sheet: fn() -> Option<u32>) {
        if self.loaded {
            return;
        }
        let Some(image) = sheet() else { return };
        let (tiles, table) = plan();
        let (cell, size) = (SHIPPED_SHEET.cell as f32, LAYER_TEX_SZ as f32);
        for (slot, tile) in tiles.iter().enumerate() {
            let mut p = Paint::new();
            p.size(LAYER_TEX_SZ, LAYER_TEX_SZ);
            p.image(image, tile.x as f32, tile.y as f32, cell, cell, 0.0, 0.0, size, size);
            p.run(g);
            g.paint_copy(self.tex, 0, 0, slot as u32, LAYER_TEX_SZ, LAYER_TEX_SZ);
        }
        mips.build(g, self.tex, self.levels, 0, self.slots);
        g.write_buffer(self.table, 0, bytes(&table));
        self.loaded = true;
    }
}

/// Layers the table holds a word for: every one a tap can name.
fn table_len() -> usize {
    GROUND_LAYER_META.len() + block_cubes().len()
}
