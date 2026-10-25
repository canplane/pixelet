// pxl-render/gpu/pack.rs

//! The texture pack as one 2D array texture with its mip chain (`mip.rs`),
//! and the table mapping a layer's top, side and bottom to array slots. A slot is a distinct
//! tile: a face with no tile of its own shares the top's slot, and a layer the
//! pack ships nothing for takes none and reads as the box's color.
//! Without the mip chain the far ground crawls as the camera moves.

use crate::frame::shader::word::{IMAGE_BOTTOM, IMAGE_SIDE, IMAGE_TOP, NO_SLOT, SLOT_BITS};
use pxl_spec::asset::ingested::{CubeFace, PACK};
use pxl_spec::asset::{shipped_texture_tile, TileOrigin};

use xpute_gpu::enums::{AddressMode, FilterMode, MipmapFilterMode};

use crate::gpu::api::{buffer_usage, bytes, release, texture_usage, Format, Gpu, Image};
use crate::gpu::mip::{mip_levels, Mips};
use crate::mesh::paint::palette::{palette, LAYERS};

pub const LAYER_TEX_SZ: u32 = PACK.cell;

const fn slot_word(top: u32, side: u32, bottom: u32) -> u32 {
    top << (IMAGE_TOP * SLOT_BITS) | side << (IMAGE_SIDE * SLOT_BITS) | bottom << (IMAGE_BOTTOM * SLOT_BITS)
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
            // stratum under it (pxl-spec-core ground.rs `Cell::block`), never a side.
            let top = shipped_texture_tile(cube, CubeFace::Top);
            let side = shipped_texture_tile(cube, CubeFace::Side).or(top);
            let bottom = shipped_texture_tile(cube, CubeFace::Bottom).or(top);
            slot_word(slot_of(&mut slots, top), slot_of(&mut slots, side), slot_of(&mut slots, bottom))
        })
        .collect();
    (slots, table)
}

pub struct Pack {
    tex: u32,
    pub view: u32,
    pub table: u32,
    pub sampler: u32,
    pub bytes: u32,
    slots: u32,
    levels: u32,
    loaded: bool,
}

impl Pack {
    /// Nothing is drawn until the pack arrives: a flat color is the box's own,
    /// which the shader already has. Until then every face reads NO_SLOT.
    pub fn new(g: &Gpu) -> Pack {
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
        Pack {
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
    pub fn step(&mut self, g: &Gpu, mips: &Mips, pack: fn() -> Option<Image>) -> bool {
        if self.loaded {
            return false;
        }
        let Some(texture) = pack().and_then(|(image, file)| g.image(image, file)) else { return false };
        let (tiles, table) = plan();
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
