// pxl-render/gpu/layers.rs

//! The texture pack as one 2D array texture with its mip chain, and the table
//! mapping a layer's top, side and bottom to array slots. A slot is a distinct
//! tile: a face with no tile of its own shares the top's slot, and a layer the
//! pack ships nothing for takes none and reads as the box's color.
//! Without the mip chain the far ground crawls as the camera moves.

use pxl_spec::asset::ingested::{CubeFace, PACK};
use pxl_spec::asset::{shipped_texture_tile, TileOrigin};

use crate::frame::shader::record::PIPELINE_MIPS;
use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{buffer_usage, bytes, pass_desc, release, sampler_entry, texture_usage, view_entry, Format, Gpu, Load};
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
    pub fn new(g: &Gpu, pipelines: &mut Pipelines, format: Format) -> Mips {
        let built = pipelines.build(g, PIPELINE_MIPS, format);
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::nearest, AddressMode::clamp_to_edge, None, 1);
        Mips {
            pipeline: built.pipeline,
            layout: built.layouts[0],
            sampler,
        }
    }

    pub fn build(&self, g: &Gpu, texture: u32, levels: u32, first_layer: u32, layers: u32) {
        let encoder = g.encoder();
        for layer in first_layer..first_layer + layers {
            for level in 1..levels {
                let above = g.view(texture, false, level - 1, 1, layer, 1);
                let target = g.view(texture, false, level, 1, layer, 1);
                let group = g.bind_group(self.layout, &[view_entry(0, above), sampler_entry(1, self.sampler)]);
                let pass = g.pass(encoder, &pass_desc(target, Load::Clear, 0, Load::Clear, 0.0, None));
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
            // A block wears what the pack ships for it: a side or a bottom of its
            // own where it has one, its top's image on every face where it has
            // not, as a cube_all. The dirt under a face is its own block, the
            // stratum under it (`pxl-spec block/sample.rs` `strata`), never a side.
            let top = shipped_texture_tile(cube, CubeFace::Top);
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
        let tex = g.texture(LAYER_TEX_SZ, LAYER_TEX_SZ, slots, Format::rgba8unorm_srgb, levels, usage);
        let view = g.view(tex, true, 0, levels, 0, slots);
        // Anisotropic 4x: the ground is mostly seen at a grazing angle, where an
        // isotropic mip blurs the tile along the view.
        let sampler = g.sampler(FilterMode::linear, FilterMode::linear, MipmapFilterMode::linear, AddressMode::repeat, None, 4);
        let blank: Vec<u32> = vec![NO_SLOTS; LAYERS];
        let table = g.buffer((blank.len() * 4) as u32, buffer_usage::STORAGE | buffer_usage::COPY_DST);
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

    /// Whether the pack came this frame.
    pub fn step(&mut self, g: &Gpu, mips: &Mips, pack: fn() -> Option<u32>) -> bool {
        if self.loaded {
            return false;
        }
        let Some(image) = pack() else { return false };
        let (tiles, table) = plan();
        let texture = g.image_texture(image);
        let encoder = g.encoder();
        for (slot, tile) in tiles.iter().enumerate() {
            g.copy_texture(encoder, texture, tile.x, tile.y, self.tex, 0, 0, slot as u32, LAYER_TEX_SZ, LAYER_TEX_SZ);
        }
        g.submit(encoder);
        release(texture);
        mips.build(g, self.tex, self.levels, 0, self.slots);
        g.write_buffer(self.table, 0, bytes(&table));
        self.loaded = true;
        true
    }
}
