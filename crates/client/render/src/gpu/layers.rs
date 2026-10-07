// pxl-render/gpu/layers.rs

//! The texture pack as one 2D array texture with its mip chain, and the table
//! mapping a layer's top, side and bottom to array slots. A slot is a distinct
//! tile: a face with no tile of its own shares the top's slot, and a layer the
//! pack ships nothing for takes none and reads as the box's color.
//! Without the mip chain the far ground crawls as the camera moves.

use pxl_spec::asset::ingested::{CubeFace, PACK};
use pxl_spec::asset::{shipped_texture_tile, TileOrigin};

use crate::frame::shader::record::PIPELINE_MIPS;
use crate::gpu::api::{buffer_usage, bytes, object, pass_desc, release, texture_usage, Gpu, Stamp};
use crate::gpu::api_spec::{address, compare, dimension, filter, format, load, resource};
use crate::gpu::pipeline::Pipelines;
use crate::mesh::paint::palette::{palette, LAYERS};

pub const LAYER_TEX_SZ: u32 = PACK.cell;

pub fn mip_levels(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

pub struct Mips {
    pipeline: u32,
    layout: u32,
    sampler: u32,
}

impl Mips {
    /// A pipeline draws into the format it was built for, so each format needs its own.
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
                g.pass(pass, encoder, &pass_desc(target, load::CLEAR, 0, load::CLEAR, 0.0, Stamp::UNTIMED));
                g.set_pipeline(pass, self.pipeline);
                g.set_bind_group(pass, 0, group, &[]);
                g.draw(pass, 3, 1, 0, 0);
                g.end(pass);
                // The encoder keeps what it recorded; letting these go now only frees the handles.
                for id in [pass, group, target, above] {
                    release(id);
                }
            }
        }
        g.submit(encoder);
        release(encoder);
    }

    pub fn release(self) {
        release(self.sampler);
    }
}

/// Ten bits, past any array WebGPU gives (maxTextureArrayLayers is 256).
const NO_SLOT: u32 = 0x3ff;
const SLOT_BITS: u32 = 10;

const fn slot_word(top: u32, side: u32, bottom: u32) -> u32 {
    top | (side << SLOT_BITS) | (bottom << (2 * SLOT_BITS))
}

const NO_SLOTS: u32 = slot_word(NO_SLOT, NO_SLOT, NO_SLOT);

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
    let table = palette()
        .map(|cube| {
            let top = shipped_texture_tile(cube, CubeFace::Top);
            // No side of its own means one image on every face, as Minecraft's cube_all.
            // The pack has no side without a bottom.
            let side = shipped_texture_tile(cube, CubeFace::Side).or(top);
            let bottom = shipped_texture_tile(cube, CubeFace::Bottom).or(top);
            slot_word(slot_of(&mut slots, top), slot_of(&mut slots, side), slot_of(&mut slots, bottom))
        })
        .collect();
    (slots, table)
}

pub struct Layers {
    tex: u32,
    pub view: u32,
    pub table: u32,
    pub sampler: u32,
    pub bytes: u32,
    slots: u32,
    levels: u32,
    loaded: bool,
}

impl Layers {
    /// Nothing is drawn until the pack arrives: a flat color is the box's own,
    /// which the shader already has. Until then every face reads NO_SLOT.
    pub fn new(g: &Gpu) -> Layers {
        let (tiles, _) = plan();
        let slots = tiles.len() as u32;
        let levels = mip_levels(LAYER_TEX_SZ, LAYER_TEX_SZ);
        let usage = texture_usage::TEXTURE_BINDING | texture_usage::COPY_DST | texture_usage::RENDER_ATTACHMENT;
        let tex = object();
        g.texture(tex, LAYER_TEX_SZ, LAYER_TEX_SZ, slots, format::RGBA8_SRGB, levels, usage);
        let view = object();
        g.view(view, tex, dimension::D2_ARRAY, 0, levels, 0, slots);
        // Anisotropic 4x: the ground is mostly seen at a grazing angle, where an
        // isotropic mip blurs the tile along the view.
        let sampler = object();
        g.sampler(sampler, filter::LINEAR, filter::LINEAR, filter::LINEAR, address::REPEAT, compare::NONE, 4);
        let table = object();
        let blank: Vec<u32> = vec![NO_SLOTS; LAYERS];
        g.buffer(table, (blank.len() * 4) as u32, buffer_usage::STORAGE | buffer_usage::COPY_DST);
        g.write_buffer(table, 0, bytes(&blank));
        // A chain is under four thirds of its first level.
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

    pub fn step(&mut self, g: &Gpu, mips: &Mips, pack: fn() -> Option<u32>) {
        if self.loaded {
            return;
        }
        let Some(image) = pack() else { return };
        let (tiles, table) = plan();
        let texture = object();
        g.image_texture(texture, image);
        let encoder = object();
        g.encoder(encoder);
        for (slot, tile) in tiles.iter().enumerate() {
            g.copy_texture(encoder, texture, tile.x, tile.y, self.tex, 0, 0, slot as u32, LAYER_TEX_SZ, LAYER_TEX_SZ);
        }
        g.submit(encoder);
        release(encoder);
        release(texture);
        mips.build(g, self.tex, self.levels, 0, self.slots);
        g.write_buffer(self.table, 0, bytes(&table));
        self.loaded = true;
    }
}
